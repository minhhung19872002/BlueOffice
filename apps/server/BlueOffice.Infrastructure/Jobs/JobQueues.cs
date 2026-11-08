using System.Collections.Concurrent;
using System.Text.Json;
using System.Threading.Channels;
using BlueOffice.Domain;
using StackExchange.Redis;

namespace BlueOffice.Infrastructure.Jobs;

/// <summary>Options of the job queue (section <c>Jobs</c>).</summary>
public sealed class JobQueueOptions
{
    /// <summary>"InMemory" (single process, tests) or "Redis".</summary>
    public string Provider { get; set; } = "InMemory";

    /// <summary>Redis key prefix.</summary>
    public string KeyPrefix { get; set; } = "bo:jobs:";

    /// <summary>How long job records are kept.</summary>
    public TimeSpan Retention { get; set; } = TimeSpan.FromDays(1);

    /// <summary>Number of jobs processed concurrently by one worker process.</summary>
    public int Concurrency { get; set; } = 2;

    /// <summary>Wait between two polls of an empty Redis queue.</summary>
    public TimeSpan PollInterval { get; set; } = TimeSpan.FromMilliseconds(500);
}

/// <summary>In-process queue (jobs are serialized so callers never share mutable instances).</summary>
public sealed class InMemoryJobQueue : IJobQueue
{
    private readonly Channel<string> _queue = Channel.CreateUnbounded<string>();
    private readonly ConcurrentDictionary<string, string> _jobs = new();

    /// <inheritdoc />
    public async Task EnqueueAsync(ConversionJob job, CancellationToken ct)
    {
        _jobs[job.Id] = JsonSerializer.Serialize(job);
        await _queue.Writer.WriteAsync(job.Id, ct);
    }

    /// <inheritdoc />
    public async Task<ConversionJob> DequeueAsync(CancellationToken ct)
    {
        while (true)
        {
            var id = await _queue.Reader.ReadAsync(ct);
            if (await GetAsync(id, ct) is { } job)
            {
                return job;
            }
        }
    }

    /// <inheritdoc />
    public Task UpdateAsync(ConversionJob job, CancellationToken ct)
    {
        _jobs[job.Id] = JsonSerializer.Serialize(job);
        return Task.CompletedTask;
    }

    /// <inheritdoc />
    public Task<ConversionJob?> GetAsync(string id, CancellationToken ct) =>
        Task.FromResult(_jobs.TryGetValue(id, out var json) ? JsonSerializer.Deserialize<ConversionJob>(json) : null);
}

/// <summary>
/// The few Redis commands <see cref="RedisJobQueue"/> needs (seam for tests; <see cref="RedisCommands"/> in production).
/// </summary>
public interface IRedisCommands
{
    /// <summary><c>LPUSH key value</c>.</summary>
    Task ListLeftPushAsync(string key, string value);

    /// <summary><c>RPOP key</c>; null when the list is empty.</summary>
    Task<string?> ListRightPopAsync(string key);

    /// <summary><c>SET key value EX ttl</c>.</summary>
    Task StringSetAsync(string key, string value, TimeSpan ttl);

    /// <summary><c>GET key</c>; null when missing or expired.</summary>
    Task<string?> StringGetAsync(string key);

    /// <summary><c>SET key value NX PX ttl</c>: true when the key was free and is now set.</summary>
    Task<bool> StringSetIfNotExistsAsync(string key, string value, TimeSpan ttl);

    /// <summary>Resets the TTL of <paramref name="key"/> only while it still holds <paramref name="value"/>
    /// (atomic compare-and-expire); false when the key expired or belongs to someone else.</summary>
    Task<bool> ExpireIfValueAsync(string key, string value, TimeSpan ttl);

    /// <summary>Deletes <paramref name="key"/> only while it holds <paramref name="value"/> (atomic compare-and-delete).</summary>
    Task<bool> DeleteIfValueAsync(string key, string value);
}

/// <summary><see cref="IRedisCommands"/> over StackExchange.Redis.</summary>
public sealed class RedisCommands(IConnectionMultiplexer redis) : IRedisCommands
{
    private IDatabase Db => redis.GetDatabase();

    /// <inheritdoc />
    public Task ListLeftPushAsync(string key, string value) => Db.ListLeftPushAsync(key, value);

    /// <inheritdoc />
    public async Task<string?> ListRightPopAsync(string key)
    {
        var v = await Db.ListRightPopAsync(key);
        return v.HasValue ? v.ToString() : null;
    }

    /// <inheritdoc />
    public Task StringSetAsync(string key, string value, TimeSpan ttl) => Db.StringSetAsync(key, value, ttl);

    /// <inheritdoc />
    public async Task<string?> StringGetAsync(string key)
    {
        var v = await Db.StringGetAsync(key);
        return v.HasValue ? v.ToString() : null;
    }

    /// <inheritdoc />
    public Task<bool> StringSetIfNotExistsAsync(string key, string value, TimeSpan ttl) =>
        Db.StringSetAsync(key, value, ttl, When.NotExists);

    /// <inheritdoc />
    public async Task<bool> ExpireIfValueAsync(string key, string value, TimeSpan ttl) =>
        (long)await Db.ScriptEvaluateAsync(
            "if redis.call('GET', KEYS[1]) == ARGV[1] then return redis.call('PEXPIRE', KEYS[1], ARGV[2]) else return 0 end",
            [key],
            [value, (long)ttl.TotalMilliseconds]) == 1;

    /// <inheritdoc />
    public async Task<bool> DeleteIfValueAsync(string key, string value) =>
        (long)await Db.ScriptEvaluateAsync(
            "if redis.call('GET', KEYS[1]) == ARGV[1] then return redis.call('DEL', KEYS[1]) else return 0 end",
            [key],
            [value]) == 1;
}

/// <summary>
/// Redis-backed queue: list <c>{prefix}queue</c> (LPUSH + RPOP = first in, first out) + one string per job record
/// (<c>{prefix}{id}</c>, expires after <see cref="JobQueueOptions.Retention"/>). A queued id whose record expired is skipped.
/// </summary>
public sealed class RedisJobQueue(IRedisCommands redis, JobQueueOptions options) : IJobQueue
{
    private string QueueKey => options.KeyPrefix + "queue";

    private string JobKey(string id) => options.KeyPrefix + id;

    /// <inheritdoc />
    public async Task EnqueueAsync(ConversionJob job, CancellationToken ct)
    {
        await UpdateAsync(job, ct);
        await redis.ListLeftPushAsync(QueueKey, job.Id);
    }

    /// <inheritdoc />
    public async Task<ConversionJob> DequeueAsync(CancellationToken ct)
    {
        while (true)
        {
            ct.ThrowIfCancellationRequested();
            var id = await redis.ListRightPopAsync(QueueKey);
            if (id is not null && await GetAsync(id, ct) is { } job)
            {
                return job;
            }

            if (id is null)
            {
                await Task.Delay(options.PollInterval, ct);
            }
        }
    }

    /// <inheritdoc />
    public Task UpdateAsync(ConversionJob job, CancellationToken ct) =>
        redis.StringSetAsync(JobKey(job.Id), JsonSerializer.Serialize(job), options.Retention);

    /// <inheritdoc />
    public async Task<ConversionJob?> GetAsync(string id, CancellationToken ct)
    {
        var v = await redis.StringGetAsync(JobKey(id));
        return v is not null ? JsonSerializer.Deserialize<ConversionJob>(v) : null;
    }
}
