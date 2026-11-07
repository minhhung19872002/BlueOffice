using System.Diagnostics;
using BlueOffice.Domain;
using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Options;

namespace BlueOffice.Infrastructure;

/// <summary>Options for <see cref="BoCliConverter"/>.</summary>
public sealed class BoCliOptions
{
    /// <summary>Path to the bo-cli executable.</summary>
    public string ExecutablePath { get; set; } = "bo-cli";

    /// <summary>Maximum time allowed per conversion.</summary>
    public TimeSpan Timeout { get; set; } = TimeSpan.FromSeconds(60);

    /// <summary>Maximum accepted input size in bytes.</summary>
    public long MaxInputBytes { get; set; } = 100 * 1024 * 1024;

    /// <summary>Input extensions accepted by the conversion worker.</summary>
    public List<string> AllowedExtensions { get; set; } = [".docx", ".txt", ".md"];
}

/// <summary>Runs <c>bo-cli convert</c> in an isolated temp directory.</summary>
public sealed class BoCliConverter(IOptions<BoCliOptions> options, ILogger<BoCliConverter> logger) : IDocumentConverter
{
    private static readonly HashSet<string> AllowedExtensions = new(StringComparer.OrdinalIgnoreCase) { ".docx", ".txt", ".md" };

    public async Task<ConversionResult> ConvertAsync(Stream input, string fileName, ConversionTarget target, CancellationToken ct)
    {
        var opts = options.Value;
        var ext = Path.GetExtension(fileName);
        if (!AllowedExtensions.Contains(ext))
        {
            throw new ConversionException($"Unsupported input type '{ext}'.");
        }

        var dir = Directory.CreateTempSubdirectory("bo-convert-");
        try
        {
            // Never trust the client file name for paths.
            var inPath = Path.Combine(dir.FullName, "input" + ext.ToLowerInvariant());
            var outPath = Path.Combine(dir.FullName, "output.pdf");
            await using (var fs = File.Create(inPath))
            {
                await CopyLimitedAsync(input, fs, opts.MaxInputBytes, ct);
            }

            var psi = new ProcessStartInfo(opts.ExecutablePath)
            {
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                UseShellExecute = false,
                WorkingDirectory = dir.FullName,
            };
            foreach (var a in new[] { "convert", inPath, "--to", "pdf", "--out", outPath })
            {
                psi.ArgumentList.Add(a);
            }

            using var proc = Process.Start(psi) ?? throw new InvalidOperationException("cannot start bo-cli");
            using var timeout = CancellationTokenSource.CreateLinkedTokenSource(ct);
            timeout.CancelAfter(opts.Timeout);
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

            if (proc.ExitCode != 0)
            {
                var err = await stderr;
                logger.LogWarning("bo-cli failed ({Code}): {Error}", proc.ExitCode, err);
                throw new ConversionException(err.Trim());
            }

            var name = Path.GetFileNameWithoutExtension(fileName) + ".pdf";
            return new ConversionResult(await File.ReadAllBytesAsync(outPath, ct), "application/pdf", name);
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

    private static async Task CopyLimitedAsync(Stream src, Stream dst, long max, CancellationToken ct)
    {
        var buffer = new byte[81920];
        long total = 0;
        int n;
        while ((n = await src.ReadAsync(buffer, ct)) > 0)
        {
            total += n;
            if (total > max)
            {
                throw new ConversionException("Input exceeds the maximum size.");
            }

            await dst.WriteAsync(buffer.AsMemory(0, n), ct);
        }
    }
}
