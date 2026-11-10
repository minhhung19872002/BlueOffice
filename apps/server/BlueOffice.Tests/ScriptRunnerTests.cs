using System.IO.Compression;
using System.Net;
using System.Net.Http.Json;
using System.Text;
using System.Text.Json;
using BlueOffice.Api.Scripts;
using BlueOffice.Api.Sessions;
using BlueOffice.Domain;
using Microsoft.Extensions.DependencyInjection;
using MockBlueDrive;
using Xunit;

namespace BlueOffice.Tests;

/// <summary>
/// Scheduled BlueScript runs executed headless (CORE-SCRIPT-004): the worker runs due schedules through
/// <c>bo-cli script run</c>, saves the result as a new host version, keeps a run history, disables a schedule after
/// repeated failures and does nothing while <c>Scripts:ScheduleEnabled</c> is off.
/// </summary>
public sealed class ScriptRunnerTests : IAsyncLifetime
{
    private readonly TestClock _clock = new();
    private MockDrive _drive = null!;

    public async Task InitializeAsync() => _drive = await MockDrive.StartAsync();

    public async Task DisposeAsync() => await _drive.DisposeAsync();

    private TestApp App(bool enabled, Dictionary<string, string?>? extra = null)
    {
        var settings = new Dictionary<string, string?>
        {
            ["Scripts:SchedulerEnabled"] = "false",
            ["Scripts:ScheduleEnabled"] = enabled ? "true" : "false",
            ["Scripts:MaxConsecutiveFailures"] = "3",
        };
        foreach (var kv in extra ?? [])
        {
            settings[kv.Key] = kv.Value;
        }

        return new TestApp(_drive, clock: _clock, settings: settings);
    }

    private async Task<(HttpClient Client, string Sid)> OpenAsync(TestApp app, string user, string fileId)
    {
        var token = await _drive.TokenAsync(user, fileId);
        var res = await app.CreateClient().PostAsJsonAsync(new Uri("/api/sessions", UriKind.Relative), new { host = "bluedrive", fileId, token });
        Assert.Equal(HttpStatusCode.Created, res.StatusCode);
        var s = await res.Content.ReadFromJsonAsync<JsonElement>();
        var c = app.CreateClient();
        c.DefaultRequestHeaders.Add(SessionEndpoints.TokenHeader, s.GetProperty("sessionToken").GetString());
        return (c, s.GetProperty("sessionId").GetString()!);
    }

    private static Uri Url(string sid, string? rest = null) =>
        new(rest is null ? $"/api/sessions/{sid}/schedules" : $"/api/sessions/{sid}/schedules/{rest}", UriKind.Relative);

    private static async Task<string> CreateAsync(HttpClient c, string sid, string name, string code, string cron = "0 7 * * *")
    {
        var res = await c.PostAsJsonAsync(Url(sid), new { name, code, cron });
        Assert.Equal(HttpStatusCode.Created, res.StatusCode);
        return (await res.Content.ReadFromJsonAsync<JsonElement>()).GetProperty("id").GetString()!;
    }

    private DateTime Due(string cron = "0 7 * * *") => CronExpression.Parse(cron).Next(_clock.GetUtcNow().UtcDateTime)!.Value;

    private static async Task<JsonElement> ScheduleAsync(HttpClient c, string sid, string id) =>
        (await c.GetFromJsonAsync<JsonElement>(Url(sid))).GetProperty("schedules").EnumerateArray().Single(x => x.GetProperty("id").GetString() == id);

    private static async Task<List<JsonElement>> RunsAsync(HttpClient c, string sid, string id) =>
        (await c.GetFromJsonAsync<JsonElement>(Url(sid, id + "/runs"))).GetProperty("runs").EnumerateArray().ToList();

    private static string Report(bool ok, bool modified, string? error = null) => JsonSerializer.Serialize(new
    {
        app = "doc",
        ok,
        error = error is null ? null : new { kind = "error", message = error, line = 2, column = 5 },
        logs = new[] { new { level = "log", text = "xin chào" } },
        logsTruncated = false,
        calls = 3,
        ms = 12,
        modified = ok ? modified : (bool?)null,
    });

    [Fact]
    public async Task DueScheduleRunsAndSavesANewVersion()
    {
        using var app = App(enabled: true);
        var id = await _drive.CreateFileAsync("Báo cáo.docx", "v1");
        var (c, sid) = await OpenAsync(app, "u1", id);
        const string code = "await doc.appendParagraph('Tổng hợp');";
        var schedule = await CreateAsync(c, sid, "Tổng hợp tuần", code);

        string? scriptSeen = null;
        byte[]? inputSeen = null;
        app.Runner.Handler = args =>
        {
            if (args[0] != "script")
            {
                return null;
            }

            Assert.Equal("run", args[1]);
            inputSeen = File.ReadAllBytes(args[2]);
            scriptSeen = File.ReadAllText(args[args.ToList().IndexOf("--script") + 1]);
            Assert.Contains("--json", args);
            Assert.Equal("30000", args[args.ToList().IndexOf("--timeout") + 1]);
            Assert.Equal("64", args[args.ToList().IndexOf("--memory") + 1]);
            File.WriteAllBytes(args[args.ToList().IndexOf("--out") + 1], "v2 by script"u8.ToArray());
            return new ProcessResult(0, Report(ok: true, modified: true), "");
        };

        var worker = app.Services.GetRequiredService<ScriptScheduleWorker>();
        var due = Due();
        Assert.Equal(1, await worker.RunDueAsync(due.AddSeconds(5), default));
        Assert.Equal(code, scriptSeen);
        Assert.Equal("v1"u8.ToArray(), inputSeen);

        // A new host version with the script name and run time as its note.
        var file = await _drive.FileStateAsync(id);
        var last = file.GetProperty("versions").EnumerateArray().Last();
        Assert.Equal("v2 by script", Encoding.UTF8.GetString(Convert.FromBase64String(last.GetProperty("content").GetString()!)));
        Assert.Equal($"BlueScript: Tổng hợp tuần ({due:yyyy-MM-dd HH:mm} UTC)", last.GetProperty("note").GetString());
        Assert.Equal("u1", last.GetProperty("modifiedBy").GetString());

        var s = await ScheduleAsync(c, sid, schedule);
        Assert.Equal(ScriptRunResult.Succeeded, s.GetProperty("lastStatus").GetString());
        Assert.Equal(0, s.GetProperty("consecutiveFailures").GetInt32());
        var run = Assert.Single(await RunsAsync(c, sid, schedule));
        Assert.Equal(ScriptRunResult.Succeeded, run.GetProperty("status").GetString());
        Assert.Equal("[log] xin chào\n", run.GetProperty("logs").GetString());
        Assert.Equal(last.GetProperty("version").GetString(), run.GetProperty("version").GetString());

        // A run that changes nothing saves no version.
        app.Runner.Handler = args => args[0] == "script" ? new ProcessResult(0, Report(ok: true, modified: false), "") : null;
        var versions = file.GetProperty("versions").GetArrayLength();
        Assert.Equal(1, await worker.RunDueAsync(due.AddDays(1).AddSeconds(5), default));
        Assert.Equal(versions, (await _drive.FileStateAsync(id)).GetProperty("versions").GetArrayLength());
        Assert.Equal(2, (await RunsAsync(c, sid, schedule)).Count);
    }

    [Fact]
    public async Task RepeatedFailuresDisableTheScheduleUntilReEnabled()
    {
        using var app = App(enabled: true);
        var id = await _drive.CreateFileAsync("Bảng.xlsx", "v1");
        var (c, sid) = await OpenAsync(app, "u1", id);
        var schedule = await CreateAsync(c, sid, "Lỗi", "throw new Error('x')", "*/10 * * * *");
        app.Runner.Handler = args => args[0] == "script" ? new ProcessResult(1, Report(ok: false, modified: false, error: "hỏng"), "error: script failed") : null;
        var worker = app.Services.GetRequiredService<ScriptScheduleWorker>();
        var versions = (await _drive.FileStateAsync(id)).GetProperty("versions").GetArrayLength();

        var at = Due("*/10 * * * *");
        for (var i = 0; i < 3; i++)
        {
            Assert.Equal(1, await worker.RunDueAsync(at.AddMinutes(10 * i).AddSeconds(1), default));
        }

        var s = await ScheduleAsync(c, sid, schedule);
        Assert.False(s.GetProperty("enabled").GetBoolean());
        Assert.Equal(3, s.GetProperty("consecutiveFailures").GetInt32());
        Assert.Contains("3 failed runs", s.GetProperty("disabledReason").GetString());
        Assert.Equal("error: hỏng (line 2, column 5)", s.GetProperty("lastError").GetString());
        var runs = await RunsAsync(c, sid, schedule);
        Assert.Equal(3, runs.Count);
        Assert.All(runs, r => Assert.Equal(ScriptRunResult.Failed, r.GetProperty("status").GetString()));

        // Disabled: no more runs, nothing saved.
        Assert.Equal(0, await worker.RunDueAsync(at.AddMinutes(30).AddSeconds(1), default));
        Assert.Equal(versions, (await _drive.FileStateAsync(id)).GetProperty("versions").GetArrayLength());

        // The owner re-enables it: counter reset, runs again.
        var res = await c.PatchAsJsonAsync(Url(sid, schedule), new { enabled = true });
        Assert.Equal(HttpStatusCode.OK, res.StatusCode);
        var patched = await res.Content.ReadFromJsonAsync<JsonElement>();
        Assert.True(patched.GetProperty("enabled").GetBoolean());
        Assert.Equal(0, patched.GetProperty("consecutiveFailures").GetInt32());
        Assert.Equal(JsonValueKind.Null, patched.GetProperty("disabledReason").ValueKind);
        app.Runner.Handler = args => args[0] == "script" ? new ProcessResult(0, Report(ok: true, modified: false), "") : null;
        var next = patched.GetProperty("nextRunAt").GetDateTime().ToUniversalTime();
        Assert.Equal(1, await worker.RunDueAsync(next.AddSeconds(1), default));
        Assert.Equal(ScriptRunResult.Succeeded, (await ScheduleAsync(c, sid, schedule)).GetProperty("lastStatus").GetString());

        // Another user cannot see the history.
        await _drive.SetPermsAsync(id, "u2", view: true, edit: true);
        var (c2, sid2) = await OpenAsync(app, "u2", id);
        Assert.Equal(HttpStatusCode.NotFound, (await c2.GetAsync(Url(sid2, schedule + "/runs"))).StatusCode);
    }

    [Fact]
    public async Task NothingRunsWhileTheAdminSwitchIsOff()
    {
        using var app = App(enabled: false);
        var id = await _drive.CreateFileAsync("Trình chiếu.pptx", "v1");
        var (c, sid) = await OpenAsync(app, "u1", id);
        var schedule = await CreateAsync(c, sid, "Tắt", "1");
        var worker = app.Services.GetRequiredService<ScriptScheduleWorker>();
        Assert.Equal(1, await worker.RunDueAsync(Due().AddSeconds(1), default));
        Assert.DoesNotContain(app.Runner.Calls, a => a[0] == "script");
        var s = await ScheduleAsync(c, sid, schedule);
        Assert.Equal(ScriptRunResult.Unavailable, s.GetProperty("lastStatus").GetString());
        Assert.Contains("Scripts:ScheduleEnabled", s.GetProperty("lastError").GetString());
        // Not a failure: the schedule stays enabled.
        Assert.Equal(0, s.GetProperty("consecutiveFailures").GetInt32());
        Assert.True(s.GetProperty("enabled").GetBoolean());
        Assert.Single((await _drive.FileStateAsync(id)).GetProperty("versions").EnumerateArray());
    }

    [Fact]
    public async Task UnsupportedFilesCannotBeScheduled()
    {
        using var app = App(enabled: true);
        var id = await _drive.CreateFileAsync("Ghi chú.txt", "v1");
        var (c, sid) = await OpenAsync(app, "u1", id);
        var res = await c.PostAsJsonAsync(Url(sid), new { name = "x", code = "1", cron = "* * * * *" });
        Assert.Equal(HttpStatusCode.BadRequest, res.StatusCode);
    }

    [Fact]
    public void ReportOfBoCliIsParsed()
    {
        var r = BoCliScriptRunner.Parse(new ProcessResult(1, "noise\n" + Report(ok: false, modified: false, error: "boom") + "\n", ""));
        Assert.NotNull(r);
        Assert.False(r!.Ok);
        Assert.Equal("error: boom (line 2, column 5)", r.Error);
        Assert.Equal("[log] xin chào\n", r.Logs);
        Assert.Null(BoCliScriptRunner.Parse(new ProcessResult(1, "", "error: bad file")));
    }

    /// <summary>The whole path with the real bo-cli (skipped when it is not built; CI builds it first).</summary>
    [Fact]
    public async Task RealBoCliEditsTheHostFile()
    {
        var cli = BoCliConverterTests.FindCli();
        if (cli is null)
        {
            return;
        }

        using var app = App(enabled: true, new() { ["BoCli:ExecutablePath"] = cli, ["BoCli:Timeout"] = "00:02:00" });
        var docx = await File.ReadAllBytesAsync(Path.Combine(BoCliConverterTests.RepoRoot(), "tests", "corpus", "doc", "phase1-sample.docx"));
        var created = await _drive.Client.PostAsJsonAsync("/_admin/files", new CreateFileRequest("Công văn.docx", "u1", Convert.ToBase64String(docx), null));
        created.EnsureSuccessStatusCode();
        var id = (await created.Content.ReadFromJsonAsync<JsonElement>()).GetProperty("id").GetString()!;
        var (c, sid) = await OpenAsync(app, "u1", id);
        var schedule = await CreateAsync(c, sid, "Thêm dòng", "await doc.appendParagraph('Dòng do lịch chạy thêm');\nconsole.log('ok');");

        var worker = app.Services.GetRequiredService<ScriptScheduleWorker>();
        Assert.Equal(1, await worker.RunDueAsync(Due().AddSeconds(1), default));
        var s = await ScheduleAsync(c, sid, schedule);
        Assert.True(s.GetProperty("lastStatus").GetString() == ScriptRunResult.Succeeded, s.ToString());
        var last = (await _drive.FileStateAsync(id)).GetProperty("versions").EnumerateArray().Last();
        using var zip = new ZipArchive(new MemoryStream(Convert.FromBase64String(last.GetProperty("content").GetString()!)));
        using var reader = new StreamReader(zip.GetEntry("word/document.xml")!.Open());
        Assert.Contains("Dòng do lịch chạy thêm", await reader.ReadToEndAsync());
        var run = Assert.Single(await RunsAsync(c, sid, schedule));
        Assert.Equal("[log] ok\n", run.GetProperty("logs").GetString());
    }
}
