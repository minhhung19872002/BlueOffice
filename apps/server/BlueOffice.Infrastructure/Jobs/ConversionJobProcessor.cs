using System.Diagnostics;
using System.Globalization;
using System.Text;
using System.Text.Json;
using BlueOffice.Domain;
using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Options;

namespace BlueOffice.Infrastructure.Jobs;

/// <summary>Starts the real bo-cli process (argument list, no shell, timeout, tree kill).</summary>
public sealed class BoCliProcessRunner(IOptions<BoCliOptions> options) : IBoCliRunner
{
    /// <inheritdoc />
    public async Task<ProcessResult> RunAsync(IReadOnlyList<string> args, string workingDirectory, CancellationToken ct)
    {
        var psi = new ProcessStartInfo(options.Value.ExecutablePath)
        {
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            UseShellExecute = false,
            WorkingDirectory = workingDirectory,
        };
        foreach (var a in args)
        {
            psi.ArgumentList.Add(a);
        }

        using var proc = Process.Start(psi) ?? throw new InvalidOperationException("cannot start bo-cli");
        using var timeout = CancellationTokenSource.CreateLinkedTokenSource(ct);
        timeout.CancelAfter(options.Value.Timeout);
        var stdout = proc.StandardOutput.ReadToEndAsync(timeout.Token);
        var stderr = proc.StandardError.ReadToEndAsync(timeout.Token);
        try
        {
            await proc.WaitForExitAsync(timeout.Token);
        }
        catch (OperationCanceledException)
        {
            proc.Kill(entireProcessTree: true);
            throw;
        }

        return new ProcessResult(proc.ExitCode, await stdout, await stderr);
    }
}

/// <summary>Executes one <see cref="ConversionJob"/>: input/output through the object store, work through bo-cli.</summary>
public sealed class ConversionJobProcessor(
    IObjectStore store,
    IBoCliRunner runner,
    IOptions<BoCliOptions> options,
    ILogger<ConversionJobProcessor> logger)
{
    /// <summary>Maximum number of pages read by text extraction.</summary>
    public const int MaxTextPages = 500;

    private static readonly ActivitySource Activity = new("BlueOffice.Jobs");

    /// <summary>Runs the job and returns it with its final status (the caller persists it).</summary>
    public async Task<ConversionJob> ProcessAsync(ConversionJob job, CancellationToken ct)
    {
        using var activity = Activity.StartActivity("job " + job.Kind);
        activity?.SetTag("job.id", job.Id);
        var dir = Directory.CreateTempSubdirectory("bo-job-");
        try
        {
            var ext = Path.GetExtension(job.FileName).ToLowerInvariant();
            if (!options.Value.AllowedExtensions.Contains(ext, StringComparer.OrdinalIgnoreCase))
            {
                return Fail(job, $"Unsupported input type '{ext}'.");
            }

            // Never trust the client file name for paths.
            var input = Path.Combine(dir.FullName, "input" + ext);
            await using (var src = await store.OpenReadAsync(job.InputKey, ct))
            await using (var fs = File.Create(input))
            {
                await src.CopyToAsync(fs, ct);
            }

            var baseName = Path.GetFileNameWithoutExtension(job.FileName);
            switch (job.Kind)
            {
                case JobKind.Convert or JobKind.Preview:
                    {
                        var to = (job.TargetFormat ?? "pdf").ToLowerInvariant();
                        if (to != "pdf")
                        {
                            return Fail(job, $"Target format '{to}' is not supported yet.");
                        }

                        var output = Path.Combine(dir.FullName, "output.pdf");
                        var r = await runner.RunAsync(["convert", input, "--to", "pdf", "--out", output], dir.FullName, ct);
                        return await FinishFileAsync(job, r, output, "application/pdf", baseName + ".pdf", ct);
                    }

                case JobKind.Thumbnail:
                    {
                        var output = Path.Combine(dir.FullName, "thumb.png");
                        var scale = Math.Clamp(job.Scale, 0.05, 4).ToString("0.###", CultureInfo.InvariantCulture);
                        var page = Math.Max(1, job.Page).ToString(CultureInfo.InvariantCulture);
                        var r = await runner.RunAsync(["render", input, "--page", page, "--scale", scale, "--out", output], dir.FullName, ct);
                        return await FinishFileAsync(job, r, output, "image/png", baseName + ".png", ct);
                    }

                case JobKind.ExtractText:
                    return await ExtractTextAsync(job, input, dir.FullName, baseName, ct);

                default:
                    return Fail(job, "Unknown job kind.");
            }
        }
        catch (Domain.ObjectNotFoundException)
        {
            return Fail(job, "Job input is missing.");
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

    private async Task<ConversionJob> ExtractTextAsync(ConversionJob job, string input, string workDir, string baseName, CancellationToken ct)
    {
        var pages = new List<string>();
        for (var p = 0; p < MaxTextPages; p++)
        {
            var r = await runner.RunAsync(["dump", input, "--page", p.ToString(CultureInfo.InvariantCulture)], workDir, ct);
            if (r.ExitCode != 0)
            {
                if (p > 0 && r.StdErr.Contains("out of range", StringComparison.OrdinalIgnoreCase))
                {
                    break;
                }

                return Fail(job, r.StdErr.Trim());
            }

            pages.Add(TextExtraction.FromDisplayList(r.StdOut));
        }

        var payload = new
        {
            text = string.Join("\n\f\n", pages),
            pages = pages.Count,
            metadata = new { fileName = job.FileName, characters = pages.Sum(x => x.Length) },
        };
        using var ms = new MemoryStream(JsonSerializer.SerializeToUtf8Bytes(payload));
        await store.PutAsync(job.OutputKey, ms, "application/json", ct);
        return Succeed(job, "application/json", baseName + ".json");
    }

    private async Task<ConversionJob> FinishFileAsync(ConversionJob job, ProcessResult r, string output, string contentType, string name, CancellationToken ct)
    {
        if (r.ExitCode != 0 || !File.Exists(output))
        {
            logger.LogWarning("bo-cli failed for job {Job} ({Code}): {Error}", job.Id, r.ExitCode, r.StdErr);
            return Fail(job, string.IsNullOrWhiteSpace(r.StdErr) ? "Conversion failed." : r.StdErr.Trim());
        }

        await using (var fs = File.OpenRead(output))
        {
            await store.PutAsync(job.OutputKey, fs, contentType, ct);
        }

        return Succeed(job, contentType, name);
    }

    private static ConversionJob Succeed(ConversionJob job, string contentType, string name)
    {
        job.Status = JobStatus.Succeeded;
        job.OutputContentType = contentType;
        job.OutputFileName = name;
        job.Error = null;
        return job;
    }

    private static ConversionJob Fail(ConversionJob job, string error)
    {
        job.Status = JobStatus.Failed;
        job.Error = error;
        return job;
    }
}

/// <summary>Plain text from a bo-cli display list (<c>{"commands":[{"op":"glyphs","text":…,"y":…}]}</c>).</summary>
public static class TextExtraction
{
    /// <summary>Concatenates the <c>text</c> of glyph runs; a change of baseline starts a new line.</summary>
    public static string FromDisplayList(string json)
    {
        using var doc = JsonDocument.Parse(json);
        if (!doc.RootElement.TryGetProperty("commands", out var commands) || commands.ValueKind != JsonValueKind.Array)
        {
            return "";
        }

        var sb = new StringBuilder();
        double? lastY = null;
        foreach (var c in commands.EnumerateArray())
        {
            if (c.ValueKind != JsonValueKind.Object
                || !c.TryGetProperty("op", out var op) || op.GetString() != "glyphs"
                || !c.TryGetProperty("text", out var text) || text.ValueKind != JsonValueKind.String)
            {
                continue;
            }

            var y = c.TryGetProperty("y", out var yv) && yv.ValueKind == JsonValueKind.Number ? yv.GetDouble() : lastY ?? 0;
            if (lastY is { } ly && Math.Abs(y - ly) > 0.5)
            {
                sb.Append('\n');
            }

            sb.Append(text.GetString());
            lastY = y;
        }

        return sb.ToString();
    }
}
