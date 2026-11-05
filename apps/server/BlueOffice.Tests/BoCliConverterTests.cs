using BlueOffice.Domain;
using BlueOffice.Infrastructure;
using Microsoft.Extensions.Logging.Abstractions;
using Microsoft.Extensions.Options;
using Xunit;

namespace BlueOffice.Tests;

public sealed class BoCliConverterTests
{
    private static string RepoRoot()
    {
        var dir = new DirectoryInfo(AppContext.BaseDirectory);
        while (dir is not null && !File.Exists(Path.Combine(dir.FullName, "Cargo.toml")))
        {
            dir = dir.Parent;
        }

        return dir?.FullName ?? throw new InvalidOperationException("repo root not found");
    }

    private static string? FindCli()
    {
        var exe = OperatingSystem.IsWindows() ? "bo-cli.exe" : "bo-cli";
        return new[] { "release", "debug" }
            .Select(p => Path.Combine(RepoRoot(), "target", p, exe))
            .FirstOrDefault(File.Exists);
    }

    [Fact]
    public async Task RejectsUnsupportedExtension()
    {
        var c = new BoCliConverter(Options.Create(new BoCliOptions()), NullLogger<BoCliConverter>.Instance);
        await Assert.ThrowsAsync<ConversionException>(() =>
            c.ConvertAsync(new MemoryStream([1]), "a.exe", ConversionTarget.Pdf, CancellationToken.None));
    }

    [Fact]
    public async Task ConvertsSampleWithRealCli()
    {
        var cli = FindCli();
        if (cli is null)
        {
            return; // bo-cli not built in this environment (CI builds it before dotnet test).
        }

        var c = new BoCliConverter(Options.Create(new BoCliOptions { ExecutablePath = cli }), NullLogger<BoCliConverter>.Instance);
        await using var input = File.OpenRead(Path.Combine(RepoRoot(), "tests", "corpus", "write", "phase1-sample.docx"));
        var result = await c.ConvertAsync(input, "phase1-sample.docx", ConversionTarget.Pdf, CancellationToken.None);
        Assert.Equal("phase1-sample.pdf", result.FileName);
        Assert.Equal("%PDF-"u8.ToArray(), result.Content.Take(5).ToArray());
    }
}
