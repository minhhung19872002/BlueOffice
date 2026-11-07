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

/// <summary>Redis-backed queue: list <c>{prefix}queue</c> (LPUSH/RPOP) + one string per job record.</summary>
public sealed class RedisJobQueue(IConnectionMultiplexer redis, JobQueueOptions options) : IJobQueue
{
    private static readonly TimeSpan PollDelay = TimeSpan.FromMilliseconds(500);

    private IDatabase Db => redis.GetDatabase();

    private RedisKey QueueKey => options.KeyPrefix + "queue";

    private RedisKey JobKey(string id) => options.KeyPrefix + id;

    /// <inheritdoc />
    public async Task EnqueueAsync(ConversionJob job, CancellationToken ct)
    {
        await UpdateAsync(job, ct);
        await Db.ListLeftPushAsync(QueueKey, job.Id);
    }

    /// <inheritdoc />
    public async Task<ConversionJob> DequeueAsync(CancellationToken ct)
    {
        while (true)
        {
            ct.ThrowIfCancellationRequested();
            var id = await Db.ListRightPopAsync(QueueKey);
            if (id.HasValue && await GetAsync(id.ToString(), ct) is { } job)
            {
                return job;
            }

            if (!id.HasValue)
            {
                await Task.Delay(PollDelay, ct);
            }
        }
    }

    /// <inheritdoc />
    public Task UpdateAsync(ConversionJob job, CancellationToken ct) =>
        Db.StringSetAsync(JobKey(job.Id), JsonSerializer.Serialize(job), options.Retention);

    /// <inheritdoc />
    public async Task<ConversionJob?> GetAsync(string id, CancellationToken ct)
    {
        var v = await Db.StringGetAsync(JobKey(id));
        return v.HasValue ? JsonSerializer.Deserialize<ConversionJob>(v.ToString()) : null;
    }
}
