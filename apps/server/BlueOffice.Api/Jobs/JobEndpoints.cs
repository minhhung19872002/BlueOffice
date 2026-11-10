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
        j.Id, j.Kind, j.Status, j.Error, $"{j.UrlPrefix}/jobs/{j.Id}", j.Status == JobStatus.Succeeded ? $"{j.UrlPrefix}/jobs/{j.Id}/result" : null, j.CreatedAt, j.CompletedAt);
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
        group.MapGet("/jobs/{id}", StatusAsync);
        group.MapGet("/jobs/{id}/result", ResultAsync);

        // Page images of a PNG preview (POST /preview with format=png): 1-based, owner only.
        group.MapGet("/jobs/{id}/pages/{page:int}", async (string id, int page, ClaimsPrincipal p, IJobQueue queue, IObjectStore store, CancellationToken ct) =>
        {
            var job = await OwnedJobAsync(id, p, queue, ct);
            if (job.Status != JobStatus.Succeeded)
            {
                return Results.Conflict(JobDto.From(job));
            }

            return job.OutputPages is { } pages && page >= 1 && page <= pages
                ? Results.Stream(await store.OpenReadAsync(job.PageKey(page), ct), "image/png", $"page-{page}.png")
                : throw ApiException.NotFound("Page");
        });
    }

    private static string PreviewFormat(string? format) => (format ?? "pdf").ToLowerInvariant() switch
    {
        "pdf" or "" => "pdf",
        "png" => "png",
        _ => throw ApiException.BadRequest("Preview 'format' must be 'pdf' or 'png'."),
    };

    /// <summary><c>GET …/jobs/{id}</c>: status of a job of the caller.</summary>
    internal static async Task<IResult> StatusAsync(string id, ClaimsPrincipal p, IJobQueue queue, CancellationToken ct) =>
        Results.Ok(JobDto.From(await OwnedJobAsync(id, p, queue, ct)));

    /// <summary><c>GET …/jobs/{id}/result</c>: output of a finished job of the caller (409 with the status otherwise).</summary>
    internal static async Task<IResult> ResultAsync(string id, HttpContext ctx, IJobQueue queue, IObjectStore store, CancellationToken ct)
    {
        var job = await OwnedJobAsync(id, ctx.User, queue, ct);
        return job.Status == JobStatus.Succeeded
            ? await OutputAsync(ctx, job, store, ct)
            : Results.Conflict(JobDto.From(job));
    }

    private static async Task<IResult> OutputAsync(HttpContext ctx, ConversionJob job, IObjectStore store, CancellationToken ct)
    {
        if (job.EditsApplied is { } applied)
        {
            ctx.Response.Headers["X-Edits-Applied"] = applied.ToString(System.Globalization.CultureInfo.InvariantCulture);
            ctx.Response.Headers["X-Edits-Unmatched"] = (job.EditsUnmatched ?? 0).ToString(System.Globalization.CultureInfo.InvariantCulture);
        }

        return Results.Stream(await store.OpenReadAsync(job.OutputKey, ct), job.OutputContentType, job.OutputFileName);
    }

    private static async Task<ConversionJob> OwnedJobAsync(string id, ClaimsPrincipal p, IJobQueue queue, CancellationToken ct)
    {
        var job = await queue.GetAsync(id, ct);
        return job is not null && job.Owner == p.PrincipalKey() ? job : throw ApiException.NotFound("Job");
    }

    /// <summary>PDF export options of a conversion (<c>pdfa</c> = 1b|2b|3b, <c>tagged</c>, <c>bookmarks</c>), validated.</summary>
    internal static void ReadPdfOptions(IFormCollection form, ConversionJob job)
    {
        var pdfa = ((string?)form["pdfa"] ?? "").Trim().ToLowerInvariant();
        if (pdfa.Length > 0)
        {
            if (!ConversionFormats.PdfALevels.Contains(pdfa))
            {
                throw ApiException.BadRequest("'pdfa' must be 1b, 2b or 3b.");
            }

            job.Options["pdfa"] = pdfa;
        }

        foreach (var flag in new[] { "tagged", "bookmarks" })
        {
            if (IsTrue(form[flag]))
            {
                job.Options[flag] = "true";
            }
        }
    }

    /// <summary>True for a form flag set to <c>true</c> / <c>1</c>.</summary>
    internal static bool IsTrue(string? value) => value is not null && (value.Equals("true", StringComparison.OrdinalIgnoreCase) || value == "1");

    /// <summary>
    /// Queues <paramref name="job"/> (its inputs are already in the object store). With <paramref name="wait"/> the
    /// result is returned directly when it is ready within <c>Worker:WaitTimeout</c>; otherwise <c>202</c> + status URL.
    /// </summary>
    internal static async Task<IResult> EnqueueAsync(HttpContext ctx, ConversionJob job, bool wait, CancellationToken ct)
    {
        var services = ctx.RequestServices;
        var queue = services.GetRequiredService<IJobQueue>();
        var store = services.GetRequiredService<IObjectStore>();
        var worker = services.GetRequiredService<Microsoft.Extensions.Options.IOptions<WorkerOptions>>().Value;
        await queue.EnqueueAsync(job, ct);
        JobMetrics.Submitted.Add(1, new KeyValuePair<string, object?>("kind", job.Kind.ToString()));

        if (wait)
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
                        return await OutputAsync(ctx, current, store, ct);
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

        return Results.Accepted($"{job.UrlPrefix}/jobs/{job.Id}", JobDto.From(job));
    }

    private static async Task<IResult> SubmitAsync(HttpContext ctx, JobKind kind, CancellationToken ct)
    {
        var services = ctx.RequestServices;
        var store = services.GetRequiredService<IObjectStore>();
        var storage = services.GetRequiredService<StorageOptions>();
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
            TargetFormat = kind switch
            {
                JobKind.Convert => ((string?)form["to"] ?? "pdf").ToLowerInvariant(),
                JobKind.Preview => PreviewFormat(form["format"]),
                _ => "pdf",
            },
            Owner = ctx.User.PrincipalKey(),
            CreatedAt = time.GetUtcNow().UtcDateTime,
        };
        if (kind == JobKind.Convert)
        {
            ReadPdfOptions(form, job);
        }

        if (int.TryParse(form["page"], out var page))
        {
            job.Page = page;
        }

        if (double.TryParse(form["scale"], System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out var scale))
        {
            job.Scale = scale;
        }
        else if (kind == JobKind.Preview)
        {
            // Page images for reading: 1 px per point (72 dpi) unless asked otherwise.
            job.Scale = 1;
        }

        await using (var s = file.OpenReadStream())
        {
            await store.PutAsync(job.InputKey, s, "application/octet-stream", ct);
        }

        return await EnqueueAsync(ctx, job, ctx.Request.Query["wait"] == "true", ct);
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
