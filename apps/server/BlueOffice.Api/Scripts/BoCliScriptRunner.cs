using System.Globalization;
using System.Text;
using System.Text.Json;
using BlueOffice.Api.Collab;
using BlueOffice.Api.Sessions;
using BlueOffice.Domain;
using BlueOffice.Infrastructure.Hosts;
using Microsoft.AspNetCore.DataProtection;
using Microsoft.Extensions.Options;

namespace BlueOffice.Api.Scripts;

/// <summary>
/// Runs a scheduled BlueScript headless (CORE-SCRIPT-004): reads the file from its host with the owner's stored
/// access token, runs <c>bo-cli script run</c> (QuickJS inside bo-cli, the same object API as the editor, no
/// filesystem / network / timers, time / memory / call limits) in an isolated temp directory, and writes the result
/// back to the host as a new version whose note is the script name and the run time.
/// </summary>
public sealed class BoCliScriptRunner(
    IBoCliRunner cli,
    HostRegistry hosts,
    IBohpClient bohp,
    IWopiClient wopi,
    LocalHostStorage local,
    RoomManager rooms,
    IHostNotifier notifier,
    ScheduleTokenProtector tokens,
    IOptions<ScriptOptions> options,
    ILogger<BoCliScriptRunner> logger) : IScriptRunner
{
    /// <summary>Extensions a schedule can run on (saved back in the same format).</summary>
    public static readonly IReadOnlyList<string> Extensions = [".docx", ".docm", ".xlsx", ".xlsm", ".pptx", ".pptm"];

    /// <inheritdoc />
    public async Task<ScriptRunResult> RunAsync(ScriptSchedule schedule, DateTime runAtUtc, CancellationToken ct)
    {
        var o = options.Value;
        var host = schedule.Host == LocalHostStorage.Host.Name ? LocalHostStorage.Host : hosts.Find(schedule.Host);
        if (host is null)
        {
            return Failed($"Host '{schedule.Host}' is no longer configured.");
        }

        IHostStorage storage = host.Protocol switch
        {
            HostProtocol.Bohp => bohp,
            HostProtocol.Wopi => wopi,
            _ => local,
        };
        var token = host.Protocol == HostProtocol.Local ? schedule.UserId : tokens.Unprotect(schedule.ProtectedToken);
        if (string.IsNullOrEmpty(token))
        {
            return Failed("No usable host access token: open the file and its schedule list to renew it.");
        }

        // Never write under an open collaborative session (its next save would conflict): try again next time.
        if (rooms.Find($"{host.Name}/{schedule.FileId}") is not null)
        {
            return new ScriptRunResult(ScriptRunResult.Skipped, "The file is open for editing; the run was put off.");
        }

        var ctx = new HostCallContext(host, token);
        try
        {
            var info = await storage.GetFileAsync(ctx, schedule.FileId, ct);
            if (!info.Permissions.Edit)
            {
                return Failed("The owner can no longer edit this file.");
            }

            var ext = Path.GetExtension(info.Name).ToLowerInvariant();
            if (!Extensions.Contains(ext))
            {
                return Failed($"Scheduled scripts run on {string.Join(", ", Extensions)} files, not '{ext}'.");
            }

            var content = await storage.GetContentsAsync(ctx, schedule.FileId, ct);
            var dir = Directory.CreateTempSubdirectory("bo-script-");
            try
            {
                // Fixed names: nothing from the file or the user becomes a path.
                var input = Path.Combine(dir.FullName, "input" + ext);
                var output = Path.Combine(dir.FullName, "output" + ext);
                var script = Path.Combine(dir.FullName, "script.js");
                await File.WriteAllBytesAsync(input, content.Content, ct);
                await File.WriteAllTextAsync(script, schedule.Code, new UTF8Encoding(false), ct);
                var res = await cli.RunAsync(
                    [
                        "script", "run", input, "--script", script, "--out", output, "--json",
                        "--timeout", ((long)o.Timeout.TotalMilliseconds).ToString(CultureInfo.InvariantCulture),
                        "--memory", o.MemoryMb.ToString(CultureInfo.InvariantCulture),
                        "--max-calls", o.MaxCalls.ToString(CultureInfo.InvariantCulture),
                    ],
                    dir.FullName,
                    ct);
                var report = Parse(res);
                if (report is null)
                {
                    logger.LogWarning("bo-cli script run gave no report ({Code}): {Error}", res.ExitCode, res.StdErr);
                    return Failed(string.IsNullOrWhiteSpace(res.StdErr) ? $"bo-cli exited with {res.ExitCode}." : res.StdErr.Trim());
                }

                var logs = report.Logs;
                if (!report.Ok)
                {
                    return new ScriptRunResult(ScriptRunResult.Failed, report.Error ?? "The script failed.") { Logs = logs };
                }

                if (!report.Modified || !File.Exists(output))
                {
                    return new ScriptRunResult(ScriptRunResult.Succeeded, null) { Logs = logs };
                }

                var note = $"BlueScript: {schedule.Name} ({runAtUtc.ToString("yyyy-MM-dd HH:mm", CultureInfo.InvariantCulture)} UTC)";
                var put = await storage.PutContentsAsync(ctx, schedule.FileId, await File.ReadAllBytesAsync(output, ct),
                    new HostPutOptions(content.Version, null, note, [schedule.UserId]), ct);
                notifier.Publish(host, HostEvents.FileSaved, schedule.FileId, new
                {
                    version = put.Version,
                    reason = "script",
                    schedule = schedule.Id,
                    editors = new[] { new { id = schedule.UserId, name = schedule.UserName } },
                });
                return new ScriptRunResult(ScriptRunResult.Succeeded, null) { Logs = logs, Version = put.Version };
            }
            finally
            {
                try
                {
                    dir.Delete(recursive: true);
                }
                catch (IOException)
                {
                    // Best effort cleanup.
                }
            }
        }
        catch (HostConflictException)
        {
            return Failed("The file changed on the host during the run; nothing was saved.");
        }
        catch (HostException e) when (e.StatusCode is 401 or 403)
        {
            return Failed($"The host refused the stored access token ({e.StatusCode}): open the file to renew it.");
        }
        catch (HostException e)
        {
            return Failed($"Host error {e.StatusCode}: {e.Message}");
        }
    }

    private static ScriptRunResult Failed(string error) => new(ScriptRunResult.Failed, error);

    /// <summary>What the runner needs from the <c>--json</c> report of <c>bo-cli script run</c>.</summary>
    public sealed record Report(bool Ok, bool Modified, string? Error, string? Logs);

    /// <summary>Parses the report printed by <c>bo-cli script run --json</c>; null when there is none.</summary>
    public static Report? Parse(ProcessResult res)
    {
        var line = res.StdOut.Split('\n', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries).LastOrDefault();
        if (line is null || !line.StartsWith('{'))
        {
            return null;
        }

        try
        {
            using var doc = JsonDocument.Parse(line);
            var r = doc.RootElement;
            var ok = r.TryGetProperty("ok", out var okEl) && okEl.ValueKind == JsonValueKind.True;
            var modified = r.TryGetProperty("modified", out var m) && m.ValueKind == JsonValueKind.True;
            string? error = null;
            if (r.TryGetProperty("error", out var e) && e.ValueKind == JsonValueKind.Object)
            {
                var kind = e.TryGetProperty("kind", out var k) ? k.GetString() : null;
                var message = e.TryGetProperty("message", out var msg) ? msg.GetString() : null;
                var at = e.TryGetProperty("line", out var l) && l.ValueKind == JsonValueKind.Number
                    ? $" (line {l.GetInt32()}{(e.TryGetProperty("column", out var c) && c.ValueKind == JsonValueKind.Number ? $", column {c.GetInt32()}" : "")})"
                    : "";
                error = $"{kind}: {message}{at}";
            }

            var logs = new StringBuilder();
            if (r.TryGetProperty("logs", out var list) && list.ValueKind == JsonValueKind.Array)
            {
                foreach (var entry in list.EnumerateArray())
                {
                    var level = entry.TryGetProperty("level", out var lv) ? lv.GetString() : "log";
                    var text = entry.TryGetProperty("text", out var t) ? t.GetString() : "";
                    logs.Append('[').Append(level).Append("] ").Append(text).Append('\n');
                }
            }

            return new Report(ok, modified, error, logs.Length > 0 ? logs.ToString() : null);
        }
        catch (JsonException)
        {
            return null;
        }
    }
}

/// <summary>Protects the host access token stored with a schedule (ASP.NET Core Data Protection).</summary>
public sealed class ScheduleTokenProtector(IDataProtectionProvider provider)
{
    private readonly IDataProtector _protector = provider.CreateProtector("BlueOffice.ScriptSchedule.HostToken.v1");

    /// <summary>Protected form of <paramref name="token"/>.</summary>
    public string Protect(string token) => _protector.Protect(token);

    /// <summary>The token, or null when missing or no longer readable (rotated / lost keys).</summary>
    public string? Unprotect(string? protectedToken)
    {
        if (string.IsNullOrEmpty(protectedToken))
        {
            return null;
        }

        try
        {
            return _protector.Unprotect(protectedToken);
        }
        catch (System.Security.Cryptography.CryptographicException)
        {
            return null;
        }
    }
}
