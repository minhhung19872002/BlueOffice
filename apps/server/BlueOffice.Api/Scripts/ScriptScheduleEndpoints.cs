using BlueOffice.Api.Common;
using BlueOffice.Api.Sessions;
using BlueOffice.Domain;
using BlueOffice.Infrastructure.Data;
using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.Options;

namespace BlueOffice.Api.Scripts;

/// <summary>Options of section <c>Scripts</c>.</summary>
public sealed class ScriptOptions
{
    /// <summary>Run the schedule worker in this process.</summary>
    public bool SchedulerEnabled { get; set; } = true;

    /// <summary>How often the worker looks for due schedules.</summary>
    public TimeSpan PollInterval { get; set; } = TimeSpan.FromMinutes(1);

    /// <summary>Schedules one user may keep for one file.</summary>
    public int MaxSchedulesPerFile { get; set; } = 20;

    /// <summary>Largest script (characters), as in the editor's script store.</summary>
    public int MaxCodeChars { get; set; } = 200_000;

    /// <summary>
    /// Administrator switch (CORE-SCRIPT-004): actually run due schedules with the headless runtime
    /// (<c>bo-cli script run</c>). Off by default: due runs are then recorded as <c>unavailable</c>.
    /// </summary>
    public bool ScheduleEnabled { get; set; }

    /// <summary>Failed runs in a row after which a schedule is disabled (0 = never).</summary>
    public int MaxConsecutiveFailures { get; set; } = 5;

    /// <summary>Wall-clock limit of one script run (keep below <c>BoCli:Timeout</c>, which bounds the process).</summary>
    public TimeSpan Timeout { get; set; } = TimeSpan.FromSeconds(30);

    /// <summary>QuickJS heap limit of one run, in MiB.</summary>
    public int MemoryMb { get; set; } = 64;

    /// <summary>API calls one run may make.</summary>
    public int MaxCalls { get; set; } = 200_000;

    /// <summary>Runs kept in the history of each schedule.</summary>
    public int MaxRunsKept { get; set; } = 50;
}

/// <summary>Body of <c>POST /api/sessions/{id}/schedules</c>.</summary>
public sealed record CreateScheduleRequest(string? Name, string? Code, string? Cron, bool? Enabled);

/// <summary>Body of <c>PATCH /api/sessions/{id}/schedules/{scheduleId}</c>.</summary>
public sealed record UpdateScheduleRequest(bool? Enabled);

/// <summary>A schedule as returned by the API (the code is not sent back).</summary>
public sealed record ScriptScheduleDto(
    string Id, string Name, string Cron, bool Enabled, DateTime? NextRunAt, DateTime? LastRunAt, string? LastStatus, string? LastError,
    int ConsecutiveFailures, string? DisabledReason)
{
    /// <summary>Maps an entity.</summary>
    public static ScriptScheduleDto From(ScriptSchedule s) =>
        new(s.Id, s.Name, s.Cron, s.Enabled, s.NextRunAt, s.LastRunAt, s.LastStatus, s.LastError, s.ConsecutiveFailures, s.DisabledReason);
}

/// <summary>One entry of a schedule's run history.</summary>
public sealed record ScriptRunDto(DateTime StartedAt, long DurationMs, string Status, string? Error, string? Logs, string? Version)
{
    /// <summary>Maps an entity.</summary>
    public static ScriptRunDto From(ScriptRun r) => new(r.StartedAt, r.DurationMs, r.Status, r.Error, r.Logs, r.Version);
}

/// <summary>
/// Scheduled BlueScript runs of the session file (CORE-SCRIPT-002): <c>GET|POST /api/sessions/{id}/schedules</c>,
/// <c>DELETE /api/sessions/{id}/schedules/{scheduleId}</c>. A user sees and changes only their own schedules of the
/// file; creating one needs edit permission. The code comes from the request (one of the user's saved scripts) —
/// never from the document.
/// </summary>
public static class ScriptScheduleEndpoints
{
    /// <summary>Maps the routes.</summary>
    public static void MapScriptScheduleEndpoints(this IEndpointRouteBuilder app)
    {
        var g = app.MapGroup("/api/sessions/{id}/schedules");
        g.MapGet("", async (string id, HttpRequest req, SessionStore store, TimeProvider time, BlueOfficeDbContext db, ScheduleTokenProtector tokens, CancellationToken ct) =>
        {
            var s = SessionEndpoints.Require(id, req, store, time);
            var list = await Mine(db, s).OrderBy(x => x.CreatedAt).ToListAsync(ct);
            if (s.Permissions.Edit && s.Host.Protocol != HostProtocol.Local && list.Count > 0)
            {
                // Scheduled runs use the owner's latest host token.
                var token = tokens.Protect(s.AccessToken);
                list.ForEach(x => x.ProtectedToken = token);
                await db.SaveChangesAsync(ct);
            }

            return Results.Ok(new { schedules = list.Select(ScriptScheduleDto.From) });
        });
        g.MapGet("/{scheduleId}/runs", async (string id, string scheduleId, HttpRequest req, SessionStore store, TimeProvider time, BlueOfficeDbContext db, CancellationToken ct) =>
        {
            var s = SessionEndpoints.Require(id, req, store, time);
            var schedule = await Mine(db, s).FirstOrDefaultAsync(x => x.Id == scheduleId, ct) ?? throw ApiException.NotFound("Schedule");
            var runs = await db.ScriptRuns.Where(r => r.ScheduleId == schedule.Id).OrderByDescending(r => r.StartedAt).Take(200).ToListAsync(ct);
            return Results.Ok(new { runs = runs.Select(ScriptRunDto.From) });
        });
        g.MapPatch("/{scheduleId}", async (string id, string scheduleId, UpdateScheduleRequest body, HttpRequest req, SessionStore store, TimeProvider time, BlueOfficeDbContext db, ScheduleTokenProtector tokens, CancellationToken ct) =>
        {
            var s = SessionEndpoints.RequireEdit(id, req, store, time);
            var schedule = await Mine(db, s).FirstOrDefaultAsync(x => x.Id == scheduleId, ct) ?? throw ApiException.NotFound("Schedule");
            if (body.Enabled is { } enabled)
            {
                schedule.Enabled = enabled;
                schedule.DisabledReason = null;
                if (enabled)
                {
                    // Re-enabling starts a fresh failure count from the next due time, with this session's token.
                    schedule.ConsecutiveFailures = 0;
                    schedule.NextRunAt = CronExpression.TryParse(schedule.Cron, out var cron) ? cron!.Next(time.GetUtcNow().UtcDateTime) : null;
                    schedule.ProtectedToken = s.Host.Protocol == HostProtocol.Local ? null : tokens.Protect(s.AccessToken);
                }
            }

            await db.SaveChangesAsync(ct);
            return Results.Ok(ScriptScheduleDto.From(schedule));
        });
        g.MapPost("", async (string id, CreateScheduleRequest body, HttpRequest req, SessionStore store, TimeProvider time, BlueOfficeDbContext db, IOptions<ScriptOptions> options, ScheduleTokenProtector tokens, CancellationToken ct) =>
        {
            var s = SessionEndpoints.RequireEdit(id, req, store, time);
            var o = options.Value;
            var name = (body.Name ?? "").Trim();
            if (name.Length is 0 or > 200)
            {
                throw ApiException.BadRequest("'name' must have 1 to 200 characters.");
            }

            if (string.IsNullOrWhiteSpace(body.Code) || body.Code.Length > o.MaxCodeChars)
            {
                throw ApiException.BadRequest($"'code' must have 1 to {o.MaxCodeChars} characters.");
            }

            if (!BoCliScriptRunner.Extensions.Contains(Path.GetExtension(s.File.Name).ToLowerInvariant()))
            {
                throw ApiException.BadRequest($"Scripts can be scheduled on {string.Join(", ", BoCliScriptRunner.Extensions)} files.");
            }

            var cronText = (body.Cron ?? "").Trim();
            if (cronText.Length > 120 || !CronExpression.TryParse(cronText, out var cron))
            {
                throw ApiException.BadRequest("'cron' must be five fields: minute hour day month weekday.");
            }

            if (await Mine(db, s).CountAsync(ct) >= o.MaxSchedulesPerFile)
            {
                throw ApiException.Conflict($"At most {o.MaxSchedulesPerFile} schedules per file.");
            }

            var now = time.GetUtcNow().UtcDateTime;
            var schedule = new ScriptSchedule
            {
                Host = s.Host.Name,
                FileId = s.FileId,
                FileName = s.File.Name,
                UserId = s.User.Id,
                UserName = s.User.Name,
                Name = name,
                Code = body.Code,
                Cron = cronText,
                Enabled = body.Enabled ?? true,
                NextRunAt = cron!.Next(now),
                CreatedAt = now,
                ProtectedToken = s.Host.Protocol == HostProtocol.Local ? null : tokens.Protect(s.AccessToken),
            };
            db.ScriptSchedules.Add(schedule);
            await db.SaveChangesAsync(ct);
            return Results.Created($"/api/sessions/{id}/schedules/{schedule.Id}", ScriptScheduleDto.From(schedule));
        });
        g.MapDelete("/{scheduleId}", async (string id, string scheduleId, HttpRequest req, SessionStore store, TimeProvider time, BlueOfficeDbContext db, CancellationToken ct) =>
        {
            var s = SessionEndpoints.Require(id, req, store, time);
            var schedule = await Mine(db, s).FirstOrDefaultAsync(x => x.Id == scheduleId, ct) ?? throw ApiException.NotFound("Schedule");
            db.ScriptSchedules.Remove(schedule);
            db.ScriptRuns.RemoveRange(db.ScriptRuns.Where(r => r.ScheduleId == schedule.Id));
            await db.SaveChangesAsync(ct);
            return Results.NoContent();
        });
    }

    private static IQueryable<ScriptSchedule> Mine(BlueOfficeDbContext db, EditorSession s) =>
        db.ScriptSchedules.Where(x => x.Host == s.Host.Name && x.FileId == s.FileId && x.UserId == s.User.Id);
}

/// <summary>
/// A runner that never executes: used when no headless runtime is wanted (tests, or a deployment that replaces
/// <see cref="IScriptRunner"/>). Runs are recorded as <see cref="ScriptRunResult.Unavailable"/>.
/// </summary>
public sealed class UnavailableScriptRunner : IScriptRunner
{
    /// <inheritdoc />
    public Task<ScriptRunResult> RunAsync(ScriptSchedule schedule, DateTime runAtUtc, CancellationToken ct) =>
        Task.FromResult(new ScriptRunResult(ScriptRunResult.Unavailable, "No server-side BlueScript runtime is configured."));
}

/// <summary>
/// Background worker: runs due schedules through <see cref="IScriptRunner"/> (only when <c>Scripts:ScheduleEnabled</c>),
/// records each run in the history, disables a schedule after <c>Scripts:MaxConsecutiveFailures</c> failed runs in a
/// row and plans the next run.
/// </summary>
public sealed class ScriptScheduleWorker(
    IServiceScopeFactory scopes,
    IScriptRunner runner,
    IOptions<ScriptOptions> options,
    TimeProvider time,
    ILogger<ScriptScheduleWorker> logger) : BackgroundService
{
    /// <summary>Longest error / log text kept per run.</summary>
    public const int MaxErrorChars = 2000;

    /// <summary>Longest console output kept per run.</summary>
    public const int MaxLogChars = 8000;

    /// <summary>Runs every schedule due at <paramref name="nowUtc"/> once; returns how many ran.</summary>
    public async Task<int> RunDueAsync(DateTime nowUtc, CancellationToken ct)
    {
        var o = options.Value;
        using var scope = scopes.CreateScope();
        var db = scope.ServiceProvider.GetRequiredService<BlueOfficeDbContext>();
        var due = await db.ScriptSchedules.Where(x => x.Enabled && x.NextRunAt != null && x.NextRunAt <= nowUtc).ToListAsync(ct);
        foreach (var s in due)
        {
            var clock = System.Diagnostics.Stopwatch.StartNew();
            ScriptRunResult result;
            if (!o.ScheduleEnabled)
            {
                result = new ScriptRunResult(ScriptRunResult.Unavailable, "Scheduled scripts are switched off on this server (Scripts:ScheduleEnabled).");
            }
            else
            {
                try
                {
                    result = await runner.RunAsync(s, nowUtc, ct);
                }
                catch (Exception e) when (e is not OperationCanceledException)
                {
                    logger.LogWarning(e, "Scheduled script {Schedule} failed", s.Id);
                    result = new ScriptRunResult(ScriptRunResult.Failed, e.Message);
                }
            }

            s.LastRunAt = nowUtc;
            s.LastStatus = result.Status;
            s.LastError = Truncate(result.Error, MaxErrorChars);
            s.NextRunAt = CronExpression.TryParse(s.Cron, out var cron) ? cron!.Next(nowUtc) : null;
            if (result.Status == ScriptRunResult.Failed)
            {
                s.ConsecutiveFailures++;
                if (o.MaxConsecutiveFailures > 0 && s.ConsecutiveFailures >= o.MaxConsecutiveFailures)
                {
                    s.Enabled = false;
                    s.DisabledReason = $"Disabled after {s.ConsecutiveFailures} failed runs in a row.";
                    logger.LogInformation("Script schedule {Schedule} disabled after {Count} failures", s.Id, s.ConsecutiveFailures);
                }
            }
            else if (result.Status == ScriptRunResult.Succeeded)
            {
                s.ConsecutiveFailures = 0;
            }

            db.ScriptRuns.Add(new ScriptRun
            {
                ScheduleId = s.Id,
                StartedAt = nowUtc,
                DurationMs = clock.ElapsedMilliseconds,
                Status = result.Status,
                Error = Truncate(result.Error, MaxErrorChars),
                Logs = Truncate(result.Logs, MaxLogChars),
                Version = result.Version,
            });
        }

        await db.SaveChangesAsync(ct);
        await PruneAsync(db, due.Select(s => s.Id).ToList(), Math.Max(1, o.MaxRunsKept), ct);
        return due.Count;
    }

    /// <summary>Keeps the newest <paramref name="keep"/> runs of each schedule.</summary>
    private static async Task PruneAsync(BlueOfficeDbContext db, List<string> schedules, int keep, CancellationToken ct)
    {
        foreach (var id in schedules)
        {
            var old = await db.ScriptRuns.Where(r => r.ScheduleId == id).OrderByDescending(r => r.StartedAt).Skip(keep).ToListAsync(ct);
            if (old.Count > 0)
            {
                db.ScriptRuns.RemoveRange(old);
            }
        }

        await db.SaveChangesAsync(ct);
    }

    private static string? Truncate(string? text, int max) => text is { Length: > 0 } t && t.Length > max ? t[..(max - 1)] + "…" : text;

    /// <inheritdoc />
    protected override async Task ExecuteAsync(CancellationToken stoppingToken)
    {
        var interval = options.Value.PollInterval > TimeSpan.Zero ? options.Value.PollInterval : TimeSpan.FromMinutes(1);
        using var timer = new PeriodicTimer(interval, time);
        try
        {
            while (await timer.WaitForNextTickAsync(stoppingToken))
            {
                try
                {
                    await RunDueAsync(time.GetUtcNow().UtcDateTime, stoppingToken);
                }
                catch (Exception e) when (e is not OperationCanceledException)
                {
                    logger.LogWarning(e, "Script schedule pass failed");
                }
            }
        }
        catch (OperationCanceledException)
        {
            // Shutting down.
        }
    }
}
