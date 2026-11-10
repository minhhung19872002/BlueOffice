using System.Net;
using System.Net.Http.Json;
using System.Text.Json;
using BlueOffice.Api.Scripts;
using BlueOffice.Api.Sessions;
using BlueOffice.Domain;
using Microsoft.Extensions.DependencyInjection;
using Xunit;

namespace BlueOffice.Tests;

/// <summary>Cron expressions of scheduled BlueScript runs (CORE-SCRIPT-002).</summary>
public sealed class CronExpressionTests
{
    private static readonly DateTime Monday = new(2026, 11, 9, 6, 59, 30, DateTimeKind.Utc);

    [Theory]
    [InlineData("0 7 * * 1-5", "2026-11-09T07:00")]
    [InlineData("*/15 * * * *", "2026-11-09T07:00")]
    [InlineData("30 6 * * *", "2026-11-10T06:30")]
    [InlineData("0 0 1 * *", "2026-12-01T00:00")]
    [InlineData("0 9 * * 0", "2026-11-15T09:00")]
    [InlineData("0 9 * * 7", "2026-11-15T09:00")]
    [InlineData("5,10 8-9 * 11 *", "2026-11-09T08:05")]
    [InlineData("0 12 13 * 5", "2026-11-13T12:00")]
    [InlineData("0 0 29 2 *", "2028-02-29T00:00")]
    public void NextRunIsTheFirstMatchingMinute(string cron, string expected)
    {
        var next = CronExpression.Parse(cron).Next(Monday);
        Assert.Equal(DateTime.Parse(expected + ":00Z", null, System.Globalization.DateTimeStyles.AdjustToUniversal), next);
        Assert.Equal(DateTimeKind.Utc, next!.Value.Kind);
    }

    [Fact]
    public void NextIsStrictlyAfterTheGivenTime()
    {
        var at = new DateTime(2026, 11, 9, 7, 0, 0, DateTimeKind.Utc);
        Assert.Equal(at.AddDays(1), CronExpression.Parse("0 7 * * *").Next(at));
    }

    [Theory]
    [InlineData("")]
    [InlineData("* * * *")]
    [InlineData("60 * * * *")]
    [InlineData("* 24 * * *")]
    [InlineData("* * 0 * *")]
    [InlineData("* * * 13 *")]
    [InlineData("* * * * 8")]
    [InlineData("5-1 * * * *")]
    [InlineData("*/0 * * * *")]
    [InlineData("a * * * *")]
    [InlineData("-1 * * * *")]
    public void MalformedExpressionsAreRejected(string cron)
    {
        Assert.False(CronExpression.TryParse(cron, out _));
        Assert.Throws<FormatException>(() => CronExpression.Parse(cron));
    }

    [Fact]
    public void ImpossibleDatesNeverFire() => Assert.Null(CronExpression.Parse("0 0 31 2 *").Next(Monday));
}

/// <summary>
/// Schedule API of the session file (<c>/api/sessions/{id}/schedules</c>) and the schedule worker (CORE-SCRIPT-002).
/// </summary>
public sealed class ScriptScheduleTests : IAsyncLifetime
{
    private readonly TestClock _clock = new();
    private MockDrive _drive = null!;
    private TestApp _app = null!;

    public async Task InitializeAsync()
    {
        _drive = await MockDrive.StartAsync();
        _app = new TestApp(_drive, clock: _clock, settings: new() { ["Scripts:SchedulerEnabled"] = "false" });
    }

    public async Task DisposeAsync()
    {
        _app.Dispose();
        await _drive.DisposeAsync();
    }

    private async Task<(HttpClient Client, string Sid)> OpenAsync(string user, string fileId)
    {
        var token = await _drive.TokenAsync(user, fileId);
        var res = await _app.CreateClient().PostAsJsonAsync(new Uri("/api/sessions", UriKind.Relative), new { host = "bluedrive", fileId, token });
        Assert.Equal(HttpStatusCode.Created, res.StatusCode);
        var s = await res.Content.ReadFromJsonAsync<JsonElement>();
        var c = _app.CreateClient();
        c.DefaultRequestHeaders.Add(SessionEndpoints.TokenHeader, s.GetProperty("sessionToken").GetString());
        return (c, s.GetProperty("sessionId").GetString()!);
    }

    private static Uri Url(string sid, string? schedule = null) =>
        new(schedule is null ? $"/api/sessions/{sid}/schedules" : $"/api/sessions/{sid}/schedules/{schedule}", UriKind.Relative);

    private static async Task<List<JsonElement>> ListAsync(HttpClient c, string sid) =>
        (await c.GetFromJsonAsync<JsonElement>(Url(sid))).GetProperty("schedules").EnumerateArray().ToList();

    [Fact]
    public async Task CreateListAndDeleteOwnSchedules()
    {
        var id = await _drive.CreateFileAsync("Báo cáo.docx", "v1");
        await _drive.SetPermsAsync(id, "u2", view: true, edit: true);
        var (c, sid) = await OpenAsync("u1", id);

        var res = await c.PostAsJsonAsync(Url(sid), new { name = "Tổng hợp", code = "await doc.appendParagraph('x');", cron = "0 7 * * 1-5" });
        Assert.Equal(HttpStatusCode.Created, res.StatusCode);
        var created = await res.Content.ReadFromJsonAsync<JsonElement>();
        Assert.Equal("Tổng hợp", created.GetProperty("name").GetString());
        Assert.Equal("0 7 * * 1-5", created.GetProperty("cron").GetString());
        var expected = CronExpression.Parse("0 7 * * 1-5").Next(_clock.GetUtcNow().UtcDateTime);
        Assert.Equal(expected, created.GetProperty("nextRunAt").GetDateTime().ToUniversalTime());
        Assert.False(created.TryGetProperty("code", out _));

        var mine = Assert.Single(await ListAsync(c, sid));
        Assert.Equal(created.GetProperty("id").GetString(), mine.GetProperty("id").GetString());

        // Another user of the same file sees only their own schedules and cannot delete this one.
        var (c2, sid2) = await OpenAsync("u2", id);
        Assert.Empty(await ListAsync(c2, sid2));
        Assert.Equal(HttpStatusCode.NotFound, (await c2.DeleteAsync(Url(sid2, mine.GetProperty("id").GetString()))).StatusCode);

        Assert.Equal(HttpStatusCode.NoContent, (await c.DeleteAsync(Url(sid, mine.GetProperty("id").GetString()))).StatusCode);
        Assert.Empty(await ListAsync(c, sid));
    }

    [Fact]
    public async Task InvalidRequestsAndViewersAreRefused()
    {
        var id = await _drive.CreateFileAsync("Bảng.xlsx", "v1");
        await _drive.SetPermsAsync(id, "u3", view: true, edit: false);
        var (c, sid) = await OpenAsync("u1", id);
        Assert.Equal(HttpStatusCode.BadRequest, (await c.PostAsJsonAsync(Url(sid), new { name = "x", code = "1", cron = "every day" })).StatusCode);
        Assert.Equal(HttpStatusCode.BadRequest, (await c.PostAsJsonAsync(Url(sid), new { name = "x", code = "", cron = "* * * * *" })).StatusCode);
        Assert.Equal(HttpStatusCode.BadRequest, (await c.PostAsJsonAsync(Url(sid), new { name = "", code = "1", cron = "* * * * *" })).StatusCode);

        var (viewer, vsid) = await OpenAsync("u3", id);
        Assert.Equal(HttpStatusCode.Forbidden, (await viewer.PostAsJsonAsync(Url(vsid), new { name = "x", code = "1", cron = "* * * * *" })).StatusCode);

        var anonymous = _app.CreateClient();
        Assert.Equal(HttpStatusCode.Unauthorized, (await anonymous.GetAsync(Url(sid))).StatusCode);
    }

    [Fact]
    public async Task WorkerRunsDueSchedulesAndPlansTheNextRun()
    {
        var id = await _drive.CreateFileAsync("Trình chiếu.pptx", "v1");
        var (c, sid) = await OpenAsync("u1", id);
        Assert.Equal(HttpStatusCode.Created, (await c.PostAsJsonAsync(Url(sid), new { name = "A", code = "1", cron = "0 7 * * *" })).StatusCode);
        Assert.Equal(HttpStatusCode.Created, (await c.PostAsJsonAsync(Url(sid), new { name = "B", code = "2", cron = "0 9 * * *", enabled = false })).StatusCode);
        var worker = _app.Services.GetRequiredService<ScriptScheduleWorker>();

        // Due at the next 07:00 after the clock.
        var due = CronExpression.Parse("0 7 * * *").Next(_clock.GetUtcNow().UtcDateTime)!.Value;
        Assert.Equal(0, await worker.RunDueAsync(due.AddMinutes(-1), default));
        var at = due.AddSeconds(10);
        Assert.Equal(1, await worker.RunDueAsync(at, default));
        var list = await ListAsync(c, sid);
        var a = list.Single(x => x.GetProperty("name").GetString() == "A");
        // No server-side BlueScript runtime yet: the run is recorded as unavailable.
        Assert.Equal(ScriptRunResult.Unavailable, a.GetProperty("lastStatus").GetString());
        Assert.Equal(due.AddDays(1), a.GetProperty("nextRunAt").GetDateTime().ToUniversalTime());
        var b = list.Single(x => x.GetProperty("name").GetString() == "B");
        Assert.Equal(JsonValueKind.Null, b.GetProperty("lastRunAt").ValueKind);
        Assert.Equal(0, await worker.RunDueAsync(at, default));
    }
}
