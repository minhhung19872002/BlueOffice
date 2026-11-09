// Validates OOXML files produced by BlueOffice with DocumentFormat.OpenXml's
// OpenXmlValidator (prompt §7 "Validation"). Usage:
//   bo-validate <file-or-dir>... [--json report.json] [--format Office2019] [--choice]
// --choice also validates the mc:Choice branch of every mc:AlternateContent (the validator itself
// only checks the Fallback): a copy of the package with each AlternateContent replaced by its Choice
// (Choices with elements outside the SDK's schemas are left to their Fallback).
// Exit code 0 when every file validates without errors, 1 otherwise.
using System.Text.Json;
using DocumentFormat.OpenXml;
using DocumentFormat.OpenXml.Packaging;
using DocumentFormat.OpenXml.Validation;

var inputs = new List<string>();
string? jsonOut = null;
var format = FileFormatVersions.Office2019;
var choice = false;
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
        case "--choice":
            choice = true;
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

// Documents, templates, macro-enabled variants and slide shows (CORE-FMT-001); the SDK
// checks the main part's content type against the package type when it opens one.
var exts = new HashSet<string>(StringComparer.OrdinalIgnoreCase)
{
    ".docx", ".docm", ".dotx", ".dotm", ".xlsx", ".xlsm", ".xltx", ".xltm",
    ".pptx", ".pptm", ".potx", ".potm", ".ppsx", ".ppsm",
};
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
    results.Add(Validate(file, validator, choice));
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

static OpenXmlPackage OpenPackage(string file, Stream stream, bool editable) =>
    Path.GetExtension(file).ToLowerInvariant() switch
    {
        ".docx" or ".docm" or ".dotx" or ".dotm" => WordprocessingDocument.Open(stream, editable),
        ".xlsx" or ".xlsm" or ".xltx" or ".xltm" => SpreadsheetDocument.Open(stream, editable),
        _ => PresentationDocument.Open(stream, editable),
    };

static FileResult Validate(string file, OpenXmlValidator validator, bool choice)
{
    try
    {
        List<ValidationIssue> errors;
        using (var stream = File.OpenRead(file))
        using (var doc = OpenPackage(file, stream, false))
        {
            errors = validator.Validate(doc)
                .Select(e => new ValidationIssue(e.Part?.Uri.ToString() ?? "", e.Path?.XPath ?? "", e.Description))
                .ToList();
        }
        if (choice)
        {
            errors.AddRange(ValidateChoices(file, validator));
        }
        return new FileResult(file, errors);
    }
    catch (Exception ex) when (ex is OpenXmlPackageException or InvalidDataException or IOException or FormatException)
    {
        return new FileResult(file, [new ValidationIssue("", "", $"cannot open package: {ex.Message}")]);
    }
}

// The package with every mc:AlternateContent replaced by the children of its first mc:Choice.
static IEnumerable<ValidationIssue> ValidateChoices(string file, OpenXmlValidator validator)
{
    using var copy = new MemoryStream();
    using (var src = File.OpenRead(file))
    {
        src.CopyTo(copy);
    }
    copy.Position = 0;
    using var doc = OpenPackage(file, copy, true);
    var changed = false;
    foreach (var part in doc.GetAllParts())
    {
        var root = part.RootElement;
        if (root is null)
        {
            continue;
        }
        var blocks = root.Descendants<AlternateContent>().Reverse().ToList();
        foreach (var ac in blocks)
        {
            var first = ac.GetFirstChild<AlternateContentChoice>();
            // A Choice with elements the SDK has no schema for (e.g. [MS-PPTX] p159:morph) cannot be
            // judged: its Fallback is what the first pass validated.
            if (first is null || first.Descendants().Any(e => e is OpenXmlUnknownElement))
            {
                continue;
            }
            foreach (var c in first.ChildElements.ToList())
            {
                ac.InsertBeforeSelf(c.CloneNode(true));
            }
            ac.Remove();
            changed = true;
        }
        if (blocks.Count > 0)
        {
            root.Save();
        }
    }
    if (!changed)
    {
        return [];
    }
    return validator.Validate(doc)
        .Select(e => new ValidationIssue(e.Part?.Uri.ToString() ?? "", "[mc:Choice] " + (e.Path?.XPath ?? ""), e.Description))
        .ToList();
}

internal sealed record ValidationIssue(string Part, string Path, string Description);

internal sealed record FileResult(string File, List<ValidationIssue> Errors);
