using System.Diagnostics.Metrics;
using System.Security.Claims;
using BlueOffice.Api.Auth;
using BlueOffice.Api.Common;
using BlueOffice.Domain;
using BlueOffice.Infrastructure.Jobs;
using BlueOffice.Infrastructure.Storage;

namespace BlueOffice.Api.Jobs;

/// <summary>Options of section <c>Worker</c>.</summary>
public sealed class WorkerOptions
{
    /// <summary>Run the conversion worker in this process.</summary>
    public bool Enabled { get; set; } = true;

    /// <summary>Maximum time <c>?wait=true</c> blocks before answering 202.</summary>
    public TimeSpan WaitTimeout { get; set; } = TimeSpan.FromSeconds(60);
}

/// <summary>Job status as returned by the API.</summary>
public sealed record JobDto(string Id, JobKind Kind, JobStatus Status, string? Error, string StatusUrl, string? ResultUrl, DateTime CreatedAt, DateTime? CompletedAt)
{
    /// <summary>Maps a job.</summary>
    public static JobDto From(ConversionJob j) => new(
        j.Id, j.Kind, j.Status, j.Error, $"/jobs/{j.Id}", j.Status == JobStatus.Succeeded ? $"/jobs/{j.Id}/result" : null, j.CreatedAt, j.CompletedAt);
}

/// <summary>
/// Auxiliary services for hosts (prompt §6A.5): <c>POST /convert</c>, <c>/thumbnail</c>, <c>/preview</c>, <c>/extract-text</c>.
/// Multipart form: <c>file</c> + optional <c>to</c>, <c>page</c>, <c>scale</c>. <c>?wait=true</c> returns the result directly.
/// </summary>
public static class JobEndpoints
{
    /// <summary>Maps the routes.</summary>
    public static void MapJobEndpoints(this IEndpointRouteBuilder app)
    {
        var group = app.MapGroup("").RequireAuthorization(AuthSetup.ServicePolicy).DisableAntiforgery();
        group.MapPost("/convert", (HttpContext ctx, CancellationToken ct) => SubmitAsync(ctx, JobKind.Convert, ct));
        group.MapPost("/thumbnail", (HttpContext ctx, CancellationToken ct) => SubmitAsync(ctx, JobKind.Thumbnail, ct));
        group.MapPost("/preview", (HttpContext ctx, CancellationToken ct) => SubmitAsync(ctx, JobKind.Preview, ct));
        group.MapPost("/extract-text", (HttpContext ctx, CancellationToken ct) => SubmitAsync(ctx, JobKind.ExtractText, ct));
        group.MapGet("/jobs/{id}", async (string id, ClaimsPrincipal p, IJobQueue queue, CancellationToken ct) =>
            Results.Ok(JobDto.From(await OwnedJobAsync(id, p, queue, ct))));
        group.MapGet("/jobs/{id}/result", async (string id, ClaimsPrincipal p, IJobQueue queue, IObjectStore store, CancellationToken ct) =>
        {
            var job = await OwnedJobAsync(id, p, queue, ct);
            return job.Status == JobStatus.Succeeded
                ? Results.Stream(await store.OpenReadAsync(job.OutputKey, ct), job.OutputContentType, job.OutputFileName)
                : Results.Conflict(JobDto.From(job));
        });
    }

    private static async Task<ConversionJob> OwnedJobAsync(string id, ClaimsPrincipal p, IJobQueue queue, CancellationToken ct)
    {
        var job = await queue.GetAsync(id, ct);
        return job is not null && job.Owner == p.PrincipalKey() ? job : throw ApiException.NotFound("Job");
    }

    private static async Task<IResult> SubmitAsync(HttpContext ctx, JobKind kind, CancellationToken ct)
    {
        var services = ctx.RequestServices;
        var queue = services.GetRequiredService<IJobQueue>();
        var store = services.GetRequiredService<IObjectStore>();
        var storage = services.GetRequiredService<StorageOptions>();
        var worker = services.GetRequiredService<Microsoft.Extensions.Options.IOptions<WorkerOptions>>().Value;
        var time = services.GetRequiredService<TimeProvider>();
        if (!ctx.Request.HasFormContentType)
        {
            throw ApiException.BadRequest("Expected multipart/form-data with a 'file' field.");
        }

        var form = await ctx.Request.ReadFormAsync(ct);
        var file = form.Files.GetFile("file") ?? throw ApiException.BadRequest("Missing 'file'.");
        if (file.Length > storage.MaxUploadBytes)
        {
            throw new PayloadTooLargeException(storage.MaxUploadBytes);
        }

        var job = new ConversionJob
        {
            Kind = kind,
            Status = JobStatus.Queued,
            FileName = Path.GetFileName(file.FileName),
            TargetFormat = kind == JobKind.Convert ? (string?)form["to"] ?? "pdf" : "pdf",
            Owner = ctx.User.PrincipalKey(),
            CreatedAt = time.GetUtcNow().UtcDateTime,
        };
        if (int.TryParse(form["page"], out var page))
        {
            job.Page = page;
        }

        if (double.TryParse(form["scale"], System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out var scale))
        {
            job.Scale = scale;
        }

        await using (var s = file.OpenReadStream())
        {
            await store.PutAsync(job.InputKey, s, "application/octet-stream", ct);
        }

        await queue.EnqueueAsync(job, ct);
        JobMetrics.Submitted.Add(1, new KeyValuePair<string, object?>("kind", kind.ToString()));

        if (ctx.Request.Query["wait"] == "true")
        {
            using var timeout = CancellationTokenSource.CreateLinkedTokenSource(ct);
            timeout.CancelAfter(worker.WaitTimeout);
            try
            {
                while (true)
                {
                    var current = await queue.GetAsync(job.Id, timeout.Token);
                    if (current?.Status == JobStatus.Succeeded)
                    {
                        return Results.Stream(await store.OpenReadAsync(current.OutputKey, ct), current.OutputContentType, current.OutputFileName);
                    }

                    if (current?.Status == JobStatus.Failed)
                    {
                        return Results.Problem(current.Error, statusCode: StatusCodes.Status422UnprocessableEntity);
                    }

                    await Task.Delay(100, timeout.Token);
                }
            }
            catch (OperationCanceledException) when (!ct.IsCancellationRequested)
            {
                // Fall through to 202.
            }
        }

        return Results.Accepted($"/jobs/{job.Id}", JobDto.From(job));
    }
}

/// <summary>Job metrics (meter "BlueOffice").</summary>
public static class JobMetrics
{
    private static readonly Meter Meter = new("BlueOffice");

    /// <summary>Jobs submitted.</summary>
    public static readonly Counter<long> Submitted = Meter.CreateCounter<long>("blueoffice.jobs.submitted");

    /// <summary>Jobs completed (tag status).</summary>
    public static readonly Counter<long> Completed = Meter.CreateCounter<long>("blueoffice.jobs.completed");
}

/// <summary>Background worker: dequeues jobs and runs them through <see cref="ConversionJobProcessor"/>.</summary>
public sealed class ConversionWorker(
    IJobQueue queue,
    ConversionJobProcessor processor,
    JobQueueOptions options,
    TimeProvider time,
    ILogger<ConversionWorker> logger) : BackgroundService
{
    /// <inheritdoc />
    protected override Task ExecuteAsync(CancellationToken stoppingToken) =>
        Task.WhenAll(Enumerable.Range(0, Math.Max(1, options.Concurrency)).Select(_ => LoopAsync(stoppingToken)));

    private async Task LoopAsync(CancellationToken ct)
    {
        while (!ct.IsCancellationRequested)
        {
            ConversionJob job;
            try
            {
                job = await queue.DequeueAsync(ct);
            }
            catch (OperationCanceledException)
            {
                return;
            }

            try
            {
                job.Status = JobStatus.Running;
                await queue.UpdateAsync(job, ct);
                job = await processor.ProcessAsync(job, ct);
            }
            catch (OperationCanceledException) when (ct.IsCancellationRequested)
            {
                return;
            }
            catch (Exception e)
            {
                logger.LogError(e, "Job {Job} crashed", job.Id);
                job.Status = JobStatus.Failed;
                job.Error = "Internal error.";
            }

            job.CompletedAt = time.GetUtcNow().UtcDateTime;
            await queue.UpdateAsync(job, CancellationToken.None);
            JobMetrics.Completed.Add(1, new KeyValuePair<string, object?>("status", job.Status.ToString()));
        }
    }
}
