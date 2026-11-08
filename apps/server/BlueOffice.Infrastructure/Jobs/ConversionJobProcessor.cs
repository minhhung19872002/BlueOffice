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

            // bo-cli writes UTF-8 (display lists, errors); never the console code page.
            StandardOutputEncoding = Encoding.UTF8,
            StandardErrorEncoding = Encoding.UTF8,
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

    /// <summary>Maximum number of page images kept by a PNG preview.</summary>
    public const int MaxPreviewPages = 50;

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
                case JobKind.Preview when string.Equals(job.TargetFormat, "png", StringComparison.OrdinalIgnoreCase):
                    return await PreviewPagesAsync(job, input, ext, dir.FullName, baseName, ct);

                case JobKind.Convert or JobKind.Preview:
                    {
                        var to = job.Kind == JobKind.Preview ? "pdf" : (job.TargetFormat ?? "pdf").ToLowerInvariant();
                        if (!ConversionFormats.TargetsFor(ext).Contains(to))
                        {
                            return Fail(job, $"Cannot convert '{ext}' to '{to}'.");
                        }

                        var output = Path.Combine(dir.FullName, "output." + to);
                        var r = await runner.RunAsync(["convert", input, "--to", to, "--out", output], dir.FullName, ct);

                        // bo-cli keeps macros of a macro-enabled input (.doc with VBA → .docm): name and type follow the package.
                        var outFormat = r.ExitCode == 0 && File.Exists(output) ? ConversionFormats.ActualFormat(to, output) : to;
                        return await FinishFileAsync(job, r, output, ConversionFormats.ContentType(outFormat), baseName + "." + outFormat, ct);
                    }

                case JobKind.Thumbnail:
                    {
                        if (!ConversionFormats.CanRender(ext))
                        {
                            return Fail(job, $"No page renderer for '{ext}' yet.");
                        }

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

    private async Task<ConversionJob> PreviewPagesAsync(ConversionJob job, string input, string ext, string workDir, string baseName, CancellationToken ct)
    {
        if (!ConversionFormats.CanRender(ext))
        {
            return Fail(job, $"No page renderer for '{ext}' yet.");
        }

        // One PNG per page (spreadsheets: printed pages). bo-cli renders every page; at most MaxPreviewPages are kept.
        var pagesDir = Directory.CreateDirectory(Path.Combine(workDir, "pages")).FullName;
        var scale = Math.Clamp(job.Scale, 0.05, 4).ToString("0.###", CultureInfo.InvariantCulture);
        var r = await runner.RunAsync(["render", input, "--out", Path.Combine(pagesDir, "page-%d.png"), "--scale", scale], workDir, ct);
        var files = Directory.EnumerateFiles(pagesDir, "page-*.png")
            .Select(f => (Path: f, Number: int.TryParse(Path.GetFileNameWithoutExtension(f)["page-".Length..], NumberStyles.None, CultureInfo.InvariantCulture, out var n) ? n : 0))
            .Where(x => x.Number > 0)
            .OrderBy(x => x.Number)
            .ToList();
        if (r.ExitCode != 0 || files.Count == 0)
        {
            logger.LogWarning("bo-cli render failed for job {Job} ({Code}): {Error}", job.Id, r.ExitCode, r.StdErr);
            return Fail(job, string.IsNullOrWhiteSpace(r.StdErr) ? "Rendering failed." : r.StdErr.Trim());
        }

        var kept = files.Take(MaxPreviewPages).ToList();
        for (var i = 0; i < kept.Count; i++)
        {
            await using var fs = File.OpenRead(kept[i].Path);
            await store.PutAsync(job.PageKey(i + 1), fs, "image/png", ct);
        }

        var payload = new
        {
            pages = kept.Count,
            totalPages = files.Count,
            images = Enumerable.Range(1, kept.Count).Select(n => $"/jobs/{job.Id}/pages/{n}").ToList(),
        };
        using var ms = new MemoryStream(JsonSerializer.SerializeToUtf8Bytes(payload));
        await store.PutAsync(job.OutputKey, ms, "application/json", ct);
        job.OutputPages = kept.Count;
        return Succeed(job, "application/json", baseName + ".json");
    }

    private async Task<ConversionJob> ExtractTextAsync(ConversionJob job, string input, string workDir, string baseName, CancellationToken ct)
    {
        var ext = Path.GetExtension(input);
        var pages = new List<string>();
        if (ConversionFormats.IsSpreadsheet(ext))
        {
            // One "page" per sheet: its cells as CSV (values, not formulas).
            var output = Path.Combine(workDir, "sheet.csv");
            for (var sheet = 1; sheet <= MaxTextPages; sheet++)
            {
                var r = await runner.RunAsync(["convert", input, "--to", "csv", "--sheet", sheet.ToString(CultureInfo.InvariantCulture), "--out", output], workDir, ct);
                if (r.ExitCode != 0)
                {
                    if (sheet > 1 && r.StdErr.Contains("out of range", StringComparison.OrdinalIgnoreCase))
                    {
                        break;
                    }

                    return Fail(job, r.StdErr.Trim());
                }

                pages.Add((await File.ReadAllTextAsync(output, Encoding.UTF8, ct)).TrimEnd('\r', '\n'));
            }
        }
        else if (!ConversionFormats.CanRender(ext))
        {
            // .ppt / .odp (no page renderer): slide texts in one block.
            var output = Path.Combine(workDir, "deck.txt");
            var r = await runner.RunAsync(["convert", input, "--to", "txt", "--out", output], workDir, ct);
            if (r.ExitCode != 0 || !File.Exists(output))
            {
                return Fail(job, r.StdErr.Trim());
            }

            pages.Add((await File.ReadAllTextAsync(output, Encoding.UTF8, ct)).TrimEnd('\r', '\n'));
        }
        else
        {
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
        }

        var payload = new
        {
            text = string.Join("\n\f\n", pages),
            pages = pages.Count,
            metadata = new
            {
                fileName = job.FileName,
                characters = pages.Sum(x => x.Length),
                kind = ConversionFormats.IsSpreadsheet(ext) ? "spreadsheet" : ConversionFormats.IsPresentation(ext) ? "presentation" : "document",
            },
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

/// <summary>Input families and output formats of <c>bo-cli convert</c> (prompt §6A.5).</summary>
public static class ConversionFormats
{
    private static readonly string[] Documents = [".docx", ".docm", ".doc", ".odt"];
    private static readonly string[] PlainText = [".txt", ".md"];
    private static readonly string[] Spreadsheets = [".xlsx", ".xlsm", ".xlsb", ".xls", ".ods"];
    private static readonly string[] Presentations = [".pptx", ".ppt", ".odp"];

    /// <summary>Every input extension the worker understands.</summary>
    public static IReadOnlyList<string> AllInputs { get; } = [.. Documents, .. PlainText, .. Spreadsheets, .. Presentations];

    /// <summary>True for workbook inputs.</summary>
    public static bool IsSpreadsheet(string ext) => Spreadsheets.Contains(ext, StringComparer.OrdinalIgnoreCase);

    /// <summary>True for presentation inputs.</summary>
    public static bool IsPresentation(string ext) => Presentations.Contains(ext, StringComparer.OrdinalIgnoreCase);

    /// <summary>Inputs with a page renderer (<c>bo-cli render</c> / <c>dump</c>); .ppt and .odp are read as text only.</summary>
    public static bool CanRender(string ext) =>
        AllInputs.Contains(ext, StringComparer.OrdinalIgnoreCase)
        && !string.Equals(ext, ".ppt", StringComparison.OrdinalIgnoreCase) && !string.Equals(ext, ".odp", StringComparison.OrdinalIgnoreCase);

    /// <summary>Formats an input can be converted to (lower case, without dot).</summary>
    public static IReadOnlyList<string> TargetsFor(string ext)
    {
        if (Documents.Contains(ext, StringComparer.OrdinalIgnoreCase))
        {
            return ["pdf", "docx", "odt", "txt"];
        }

        if (IsSpreadsheet(ext))
        {
            return ["pdf", "xlsx", "ods", "csv"];
        }

        if (IsPresentation(ext))
        {
            // .ppt / .odp have no PresentationML writer path yet (bo-cli converts them to .odp / text only).
            return string.Equals(ext, ".pptx", StringComparison.OrdinalIgnoreCase) ? ["pdf", "pptx"] : ["odp", "txt"];
        }

        return PlainText.Contains(ext, StringComparer.OrdinalIgnoreCase) ? ["pdf"] : [];
    }

    /// <summary>
    /// Format of a written OOXML package: <paramref name="requested"/>, or its macro-enabled variant (docm, xlsm, pptm) when
    /// the main part has the macro-enabled content type (ECMA-376 Part 2 §10.1.2.2 [Content_Types].xml; [MS-OFFMACRO2]).
    /// </summary>
    public static string ActualFormat(string requested, string path)
    {
        if (requested is not ("docx" or "xlsx" or "pptx"))
        {
            return requested;
        }

        try
        {
            using var zip = System.IO.Compression.ZipFile.OpenRead(path);
            var entry = zip.GetEntry("[Content_Types].xml");
            if (entry is null)
            {
                return requested;
            }

            using var reader = new StreamReader(entry.Open(), Encoding.UTF8);
            var types = reader.ReadToEnd();
            return requested switch
            {
                "docx" when types.Contains("application/vnd.ms-word.document.macroEnabled.main+xml", StringComparison.OrdinalIgnoreCase) => "docm",
                "xlsx" when types.Contains("application/vnd.ms-excel.sheet.macroEnabled.main+xml", StringComparison.OrdinalIgnoreCase) => "xlsm",
                "pptx" when types.Contains("application/vnd.ms-powerpoint.presentation.macroEnabled.main+xml", StringComparison.OrdinalIgnoreCase) => "pptm",
                _ => requested,
            };
        }
        catch (InvalidDataException)
        {
            return requested;
        }
    }

    /// <summary>MIME type of an output format.</summary>
    public static string ContentType(string format) => format switch
    {
        "docm" => "application/vnd.ms-word.document.macroEnabled.12",
        "xlsm" => "application/vnd.ms-excel.sheet.macroEnabled.12",
        "pptm" => "application/vnd.ms-powerpoint.presentation.macroEnabled.12",
        "pdf" => "application/pdf",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "odt" => "application/vnd.oasis.opendocument.text",
        "ods" => "application/vnd.oasis.opendocument.spreadsheet",
        "odp" => "application/vnd.oasis.opendocument.presentation",
        "csv" => "text/csv",
        "txt" => "text/plain",
        "png" => "image/png",
        _ => "application/octet-stream",
    };
}
