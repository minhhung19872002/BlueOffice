using BlueOffice.Domain;
using BlueOffice.Infrastructure.Jobs;
using StackExchange.Redis;
using Testcontainers.Redis;
using Xunit;
using Xunit.Abstractions;

namespace BlueOffice.Tests;

/// <summary>In-memory stand-in for the Redis commands used by <see cref="RedisJobQueue"/> (lists + strings with TTL).</summary>
public sealed class FakeRedis(TimeProvider clock) : IRedisCommands
{
    private readonly Dictionary<string, LinkedList<string>> _lists = [];
    private readonly Dictionary<string, (string Value, DateTimeOffset Expires)> _strings = [];

    /// <summary>TTL of the last <c>SET</c> per key.</summary>
    public Dictionary<string, TimeSpan> Ttls { get; } = [];

    /// <summary>Every key ever written.</summary>
    public HashSet<string> Keys { get; } = [];

    /// <inheritdoc />
    public Task ListLeftPushAsync(string key, string value)
    {
        lock (this)
        {
            Keys.Add(key);
            if (!_lists.TryGetValue(key, out var l))
            {
                _lists[key] = l = new LinkedList<string>();
            }

            l.AddFirst(value);
        }

        return Task.CompletedTask;
    }

    /// <inheritdoc />
    public Task<string?> ListRightPopAsync(string key)
    {
        lock (this)
        {
            if (!_lists.TryGetValue(key, out var l) || l.Last is null)
            {
                return Task.FromResult<string?>(null);
            }

            var v = l.Last.Value;
            l.RemoveLast();
            return Task.FromResult<string?>(v);
        }
    }

    /// <inheritdoc />
    public Task StringSetAsync(string key, string value, TimeSpan ttl)
    {
        lock (this)
        {
            Keys.Add(key);
            Ttls[key] = ttl;
            _strings[key] = (value, clock.GetUtcNow() + ttl);
        }

        return Task.CompletedTask;
    }

    /// <inheritdoc />
    public Task<string?> StringGetAsync(string key)
    {
        lock (this)
        {
            return Task.FromResult(_strings.TryGetValue(key, out var e) && e.Expires > clock.GetUtcNow() ? e.Value : null);
        }
    }
}

/// <summary><see cref="RedisJobQueue"/>: logic with <see cref="FakeRedis"/>, then against a real Redis (Testcontainers) when Docker is reachable.</summary>
public sealed class RedisJobQueueTests(ITestOutputHelper output)
{
    private static ConversionJob Job(string name) => new() { Kind = JobKind.Convert, Status = JobStatus.Queued, FileName = name, TargetFormat = "pdf", Owner = "user:a" };

    private static JobQueueOptions Options(string prefix = "test:jobs:") =>
        new() { KeyPrefix = prefix, Retention = TimeSpan.FromHours(2), PollInterval = TimeSpan.FromMilliseconds(10) };

    [Fact]
    public async Task JobsComeOutInOrderUnderTheConfiguredPrefixWithRetention()
    {
        var clock = new TestClock();
        var redis = new FakeRedis(clock);
        var q = new RedisJobQueue(redis, Options("bo:x:"));
        var a = Job("a.docx");
        var b = Job("b.xlsx");
        await q.EnqueueAsync(a, CancellationToken.None);
        await q.EnqueueAsync(b, CancellationToken.None);

        Assert.Equal(new[] { "bo:x:queue", "bo:x:" + a.Id, "bo:x:" + b.Id }.Order(StringComparer.Ordinal), redis.Keys.Order(StringComparer.Ordinal));
        Assert.All(redis.Ttls.Values, t => Assert.Equal(TimeSpan.FromHours(2), t));
        Assert.Equal(a.Id, (await q.DequeueAsync(CancellationToken.None)).Id);
        var second = await q.DequeueAsync(CancellationToken.None);
        Assert.Equal(b.Id, second.Id);
        Assert.Equal("b.xlsx", second.FileName);

        // Status changes are persisted (and refresh the retention).
        second.Status = JobStatus.Succeeded;
        second.OutputPages = 3;
        clock.Advance(TimeSpan.FromHours(1));
        await q.UpdateAsync(second, CancellationToken.None);
        clock.Advance(TimeSpan.FromHours(1.5));
        var loaded = await q.GetAsync(b.Id, CancellationToken.None);
        Assert.Equal(JobStatus.Succeeded, loaded?.Status);
        Assert.Equal(3, loaded?.OutputPages);

        // Records expire after the retention.
        Assert.Null(await q.GetAsync(a.Id, CancellationToken.None));
        Assert.Null(await q.GetAsync("missing", CancellationToken.None));
    }

    [Fact]
    public async Task ExpiredRecordsAreSkippedAndDequeueWaitsForWork()
    {
        var clock = new TestClock();
        var redis = new FakeRedis(clock);
        var q = new RedisJobQueue(redis, Options());
        var stale = Job("old.docx");
        await q.EnqueueAsync(stale, CancellationToken.None);
        clock.Advance(TimeSpan.FromHours(3));

        // Nothing usable yet: Dequeue polls until a job arrives.
        var pending = q.DequeueAsync(CancellationToken.None);
        await Task.Delay(50);
        Assert.False(pending.IsCompleted);
        var fresh = Job("new.docx");
        await q.EnqueueAsync(fresh, CancellationToken.None);
        Assert.Equal(fresh.Id, (await pending.WaitAsync(TimeSpan.FromSeconds(5))).Id);

        using var cts = new CancellationTokenSource(TimeSpan.FromMilliseconds(100));
        await Assert.ThrowsAnyAsync<OperationCanceledException>(() => q.DequeueAsync(cts.Token));
    }

    [Fact]
    public async Task WorksAgainstARealRedisWhenDockerIsAvailable()
    {
        RedisContainer container;
        try
        {
            container = new RedisBuilder().WithImage("redis:7-alpine").Build();
            await container.StartAsync().WaitAsync(TimeSpan.FromMinutes(3));
        }
        catch (Exception e) when (e is not OutOfMemoryException)
        {
            // No Docker daemon (or no image available offline): this integration test does not apply here.
            output.WriteLine($"Skipped: Docker/Redis unavailable ({e.GetType().Name}: {e.Message})");
            return;
        }

        await using (container)
        {
            using var mux = await ConnectionMultiplexer.ConnectAsync(container.GetConnectionString());
            var options = Options("it:" + Guid.NewGuid().ToString("N")[..8] + ":");
            var q = new RedisJobQueue(new RedisCommands(mux), options);
            var a = Job("a.docx");
            var b = Job("b.pptx");
            await q.EnqueueAsync(a, CancellationToken.None);
            await q.EnqueueAsync(b, CancellationToken.None);
            Assert.Equal(a.Id, (await q.DequeueAsync(CancellationToken.None)).Id);
            Assert.Equal(b.Id, (await q.DequeueAsync(CancellationToken.None)).Id);

            b.Status = JobStatus.Failed;
            b.Error = "lỗi";
            await q.UpdateAsync(b, CancellationToken.None);
            Assert.Equal("lỗi", (await q.GetAsync(b.Id, CancellationToken.None))?.Error);

            var ttl = await mux.GetDatabase().KeyTimeToLiveAsync(options.KeyPrefix + b.Id);
            Assert.NotNull(ttl);
            Assert.InRange(ttl.Value, TimeSpan.FromHours(1.9), TimeSpan.FromHours(2));

            // A queued id without a record (expired) is skipped.
            await mux.GetDatabase().ListLeftPushAsync(options.KeyPrefix + "queue", "gone");
            var c = Job("c.xlsx");
            await q.EnqueueAsync(c, CancellationToken.None);
            Assert.Equal(c.Id, (await q.DequeueAsync(CancellationToken.None).WaitAsync(TimeSpan.FromSeconds(10))).Id);
        }
    }
}
