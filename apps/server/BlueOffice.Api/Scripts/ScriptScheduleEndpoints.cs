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
}

/// <summary>Body of <c>POST /api/sessions/{id}/schedules</c>.</summary>
public sealed record CreateScheduleRequest(string? Name, string? Code, string? Cron, bool? Enabled);

/// <summary>A schedule as returned by the API (the code is not sent back).</summary>
public sealed record ScriptScheduleDto(
    string Id, string Name, string Cron, bool Enabled, DateTime? NextRunAt, DateTime? LastRunAt, string? LastStatus, string? LastError)
{
    /// <summary>Maps an entity.</summary>
    public static ScriptScheduleDto From(ScriptSchedule s) =>
        new(s.Id, s.Name, s.Cron, s.Enabled, s.NextRunAt, s.LastRunAt, s.LastStatus, s.LastError);
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
        g.MapGet("", async (string id, HttpRequest req, SessionStore store, TimeProvider time, BlueOfficeDbContext db, CancellationToken ct) =>
        {
            var s = SessionEndpoints.Require(id, req, store, time);
            var list = await Mine(db, s).OrderBy(x => x.CreatedAt).ToListAsync(ct);
            return Results.Ok(new { schedules = list.Select(ScriptScheduleDto.From) });
        });
        g.MapPost("", async (string id, CreateScheduleRequest body, HttpRequest req, SessionStore store, TimeProvider time, BlueOfficeDbContext db, IOptions<ScriptOptions> options, CancellationToken ct) =>
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
            await db.SaveChangesAsync(ct);
            return Results.NoContent();
        });
    }

    private static IQueryable<ScriptSchedule> Mine(BlueOfficeDbContext db, EditorSession s) =>
        db.ScriptSchedules.Where(x => x.Host == s.Host.Name && x.FileId == s.FileId && x.UserId == s.User.Id);
}

/// <summary>
/// The runner used until a server-side BlueScript runtime exists: BlueScript runs in QuickJS inside the editor
/// (ADR-0012) and the server has only the bo-cli worker, which has no JavaScript engine. Runs are recorded as
/// <see cref="ScriptRunResult.Unavailable"/> instead of being executed.
/// </summary>
public sealed class UnavailableScriptRunner : IScriptRunner
{
    /// <inheritdoc />
    public Task<ScriptRunResult> RunAsync(ScriptSchedule schedule, CancellationToken ct) =>
        Task.FromResult(new ScriptRunResult(ScriptRunResult.Unavailable, "No server-side BlueScript runtime is configured."));
}

/// <summary>Background worker: runs due schedules through <see cref="IScriptRunner"/> and plans their next run.</summary>
public sealed class ScriptScheduleWorker(
    IServiceScopeFactory scopes,
    IScriptRunner runner,
    IOptions<ScriptOptions> options,
    TimeProvider time,
    ILogger<ScriptScheduleWorker> logger) : BackgroundService
{
    /// <summary>Runs every schedule due at <paramref name="nowUtc"/> once; returns how many ran.</summary>
    public async Task<int> RunDueAsync(DateTime nowUtc, CancellationToken ct)
    {
        using var scope = scopes.CreateScope();
        var db = scope.ServiceProvider.GetRequiredService<BlueOfficeDbContext>();
        var due = await db.ScriptSchedules.Where(x => x.Enabled && x.NextRunAt != null && x.NextRunAt <= nowUtc).ToListAsync(ct);
        foreach (var s in due)
        {
            ScriptRunResult result;
            try
            {
                result = await runner.RunAsync(s, ct);
            }
            catch (Exception e) when (e is not OperationCanceledException)
            {
                logger.LogWarning(e, "Scheduled script {Schedule} failed", s.Id);
                result = new ScriptRunResult(ScriptRunResult.Failed, e.Message);
            }

            s.LastRunAt = nowUtc;
            s.LastStatus = result.Status;
            s.LastError = result.Error is { Length: > 2000 } text ? text[..2000] : result.Error;
            s.NextRunAt = CronExpression.TryParse(s.Cron, out var cron) ? cron!.Next(nowUtc) : null;
        }

        await db.SaveChangesAsync(ct);
        return due.Count;
    }

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
