// Validates OOXML files produced by BlueOffice with DocumentFormat.OpenXml's
// OpenXmlValidator (prompt §7 "Validation"). Usage:
//   bo-validate <file-or-dir>... [--json report.json] [--format Office2019]
// Exit code 0 when every file validates without errors, 1 otherwise.
using System.Text.Json;
using DocumentFormat.OpenXml;
using DocumentFormat.OpenXml.Packaging;
using DocumentFormat.OpenXml.Validation;

var inputs = new List<string>();
string? jsonOut = null;
var format = FileFormatVersions.Office2019;
for (var i = 0; i < args.Length; i++)
{
    switch (args[i])
    {
        case "--json" when i + 1 < args.Length:
            jsonOut = args[++i];
            break;
        case "--format" when i + 1 < args.Length:
            format = Enum.Parse<FileFormatVersions>(args[++i]);
            break;
        default:
            inputs.Add(args[i]);
            break;
    }
}

if (inputs.Count == 0)
{
    Console.Error.WriteLine("usage: bo-validate <file-or-dir>... [--json report.json] [--format Office2019]");
    return 2;
}

var exts = new HashSet<string>(StringComparer.OrdinalIgnoreCase) { ".docx", ".xlsx", ".pptx", ".docm", ".xlsm", ".pptm" };
var files = inputs
    .SelectMany(p => Directory.Exists(p)
        ? Directory.EnumerateFiles(p, "*", SearchOption.AllDirectories)
        : [p])
    .Where(f => exts.Contains(Path.GetExtension(f)))
    .Order(StringComparer.Ordinal)
    .ToList();

var validator = new OpenXmlValidator(format);
var results = new List<FileResult>();
foreach (var file in files)
{
    results.Add(Validate(file, validator));
}

var failed = results.Count(r => r.Errors.Count > 0);
foreach (var r in results)
{
    Console.WriteLine($"{(r.Errors.Count == 0 ? "OK  " : "FAIL")} {r.File} ({r.Errors.Count} errors)");
    foreach (var e in r.Errors.Take(10))
    {
        Console.WriteLine($"     {e.Part} {e.Path}: {e.Description}");
    }
}
Console.WriteLine($"{results.Count - failed}/{results.Count} files valid");

if (jsonOut is not null)
{
    File.WriteAllText(jsonOut, JsonSerializer.Serialize(results, new JsonSerializerOptions { WriteIndented = true }));
}
return failed == 0 ? 0 : 1;

static FileResult Validate(string file, OpenXmlValidator validator)
{
    try
    {
        using OpenXmlPackage doc = Path.GetExtension(file).ToLowerInvariant() switch
        {
            ".docx" or ".docm" => WordprocessingDocument.Open(file, false),
            ".xlsx" or ".xlsm" => SpreadsheetDocument.Open(file, false),
            _ => PresentationDocument.Open(file, false),
        };
        var errors = validator.Validate(doc)
            .Select(e => new ValidationIssue(e.Part?.Uri.ToString() ?? "", e.Path?.XPath ?? "", e.Description))
            .ToList();
        return new FileResult(file, errors);
    }
    catch (Exception ex) when (ex is OpenXmlPackageException or InvalidDataException or IOException or FormatException)
    {
        return new FileResult(file, [new ValidationIssue("", "", $"cannot open package: {ex.Message}")]);
    }
}

internal sealed record ValidationIssue(string Part, string Path, string Description);

internal sealed record FileResult(string File, List<ValidationIssue> Errors);
