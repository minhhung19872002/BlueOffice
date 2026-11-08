using System.IO.Compression;
using System.Net;
using System.Net.Http.Headers;
using System.Net.Http.Json;
using System.Text.Json;
using Xunit;

namespace BlueOffice.Tests;

/// <summary>
/// <c>/convert</c>, <c>/thumbnail</c>, <c>/preview</c>, <c>/extract-text</c> through the queue, the worker and the real
/// <c>bo-cli</c> (prompt §6A.5). Skipped (returns early) when bo-cli is not built; CI builds it before <c>dotnet test</c>.
/// </summary>
public sealed class BoCliEndToEndTests : IDisposable
{
    private readonly string? _cli = BoCliConverterTests.FindCli();
    private readonly TestApp? _app;

    public BoCliEndToEndTests()
    {
        if (_cli is not null)
        {
            _app = new TestApp(settings: new() { ["BoCli:ExecutablePath"] = _cli, ["BoCli:Timeout"] = "00:02:00", ["Worker:WaitTimeout"] = "00:02:00" });
        }
    }

    public void Dispose() => _app?.Dispose();

    private static string Corpus(string relative) => Path.Combine(BoCliConverterTests.RepoRoot(), "tests", "corpus", relative);

    // Legacy binaries written by the bo-binary test builders (synthetic, no third-party content).
    private static string Fixture(string name) => Path.Combine(BoCliConverterTests.RepoRoot(), "apps", "server", "BlueOffice.Tests", "Fixtures", name);

    private async Task<HttpResponseMessage> PostAsync(string endpoint, byte[] content, string fileName, params (string Key, string Value)[] fields)
    {
        using var form = new MultipartFormDataContent();
        var file = new ByteArrayContent(content);
        file.Headers.ContentType = new MediaTypeHeaderValue("application/octet-stream");
        form.Add(file, "file", fileName);
        foreach (var (k, v) in fields)
        {
            form.Add(new StringContent(v), k);
        }

        return await _app!.ClientFor("bo-cli-e2e").PostAsync(new Uri(endpoint + "?wait=true", UriKind.Relative), form);
    }

    private async Task<byte[]> OkBytesAsync(HttpResponseMessage res, string contentType)
    {
        Assert.True(res.StatusCode == HttpStatusCode.OK, $"{(int)res.StatusCode}: {await res.Content.ReadAsStringAsync()}");
        Assert.Equal(contentType, res.Content.Headers.ContentType?.MediaType);
        return await res.Content.ReadAsByteArrayAsync();
    }

    private static void AssertPdf(byte[] b) => Assert.Equal("%PDF-"u8.ToArray(), b.Take(5).ToArray());

    private static void AssertPng(byte[] b) => Assert.Equal(new byte[] { 0x89, 0x50, 0x4E, 0x47 }, b.Take(4).ToArray());

    private static void AssertZipWith(byte[] b, string entry)
    {
        using var zip = new ZipArchive(new MemoryStream(b));
        Assert.Contains(zip.Entries, e => e.FullName == entry);
    }

    [Fact]
    public async Task ConvertsDocumentsSpreadsheetsAndPresentations()
    {
        if (_app is null)
        {
            return;
        }

        var docx = await File.ReadAllBytesAsync(Corpus("doc/phase1-sample.docx"));
        AssertPdf(await OkBytesAsync(await PostAsync("/convert", docx, "Công văn.docx", ("to", "pdf")), "application/pdf"));

        // docx → odt → docx (ODF 1.3 and back).
        var odt = await OkBytesAsync(await PostAsync("/convert", docx, "Công văn.docx", ("to", "odt")), "application/vnd.oasis.opendocument.text");
        AssertZipWith(odt, "content.xml");
        var back = await OkBytesAsync(await PostAsync("/convert", odt, "Công văn.odt", ("to", "docx")), "application/vnd.openxmlformats-officedocument.wordprocessingml.document");
        AssertZipWith(back, "word/document.xml");

        var xlsx = await File.ReadAllBytesAsync(Corpus("sheet/values-1.xlsx"));
        AssertPdf(await OkBytesAsync(await PostAsync("/convert", xlsx, "Số liệu.xlsx", ("to", "pdf")), "application/pdf"));
        var ods = await OkBytesAsync(await PostAsync("/convert", xlsx, "Số liệu.xlsx", ("to", "ods")), "application/vnd.oasis.opendocument.spreadsheet");
        AssertZipWith(await OkBytesAsync(await PostAsync("/convert", ods, "Số liệu.ods", ("to", "xlsx")), "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"), "xl/workbook.xml");
        var csv = System.Text.Encoding.UTF8.GetString(await OkBytesAsync(await PostAsync("/convert", xlsx, "Số liệu.xlsx", ("to", "csv")), "text/csv"));
        Assert.Contains("Chuỗi 1", csv, StringComparison.Ordinal);

        var pptx = await File.ReadAllBytesAsync(Corpus("slide/slide-text-bullets.pptx"));
        AssertPdf(await OkBytesAsync(await PostAsync("/convert", pptx, "Báo cáo.pptx", ("to", "pdf")), "application/pdf"));

        // Plain text → PDF (preview of a .md file).
        AssertPdf(await OkBytesAsync(await PostAsync("/preview", "# Tiêu đề\nXin chào"u8.ToArray(), "ghi chú.md"), "application/pdf"));

        // Combinations bo-cli cannot do are refused before running it.
        var refused = await PostAsync("/convert", docx, "a.docx", ("to", "xlsx"));
        Assert.Equal(HttpStatusCode.UnprocessableEntity, refused.StatusCode);
    }

    [Fact]
    public async Task ConvertsLegacyAndOpenFormatsToEveryTargetBoCliSupports()
    {
        if (_app is null)
        {
            return;
        }

        const string Docx = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";
        const string Xlsx = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";
        const string Pptx = "application/vnd.openxmlformats-officedocument.presentationml.presentation";

        // .doc → .docx (or .docm when the input carries VBA: the content type tells which).
        var doc = await File.ReadAllBytesAsync(Fixture("sample.doc"));
        var fromDoc = await PostAsync("/convert", doc, "Công văn cũ.doc", ("to", "docx"));
        Assert.Equal(HttpStatusCode.OK, fromDoc.StatusCode);
        var docType = fromDoc.Content.Headers.ContentType?.MediaType;
        Assert.True(docType is Docx or "application/vnd.ms-word.document.macroEnabled.12", docType);
        AssertZipWith(await fromDoc.Content.ReadAsByteArrayAsync(), "word/document.xml");
        AssertPdf(await OkBytesAsync(await PostAsync("/convert", doc, "Công văn cũ.doc", ("to", "pdf")), "application/pdf"));

        var docx = await File.ReadAllBytesAsync(Corpus("doc/phase1-sample.docx"));
        AssertZipWith(await OkBytesAsync(await PostAsync("/convert", docx, "a.docx", ("to", "odt")), "application/vnd.oasis.opendocument.text"), "content.xml");
        var txt = System.Text.Encoding.UTF8.GetString(await OkBytesAsync(await PostAsync("/convert", docx, "a.docx", ("to", "txt")), "text/plain"));
        Assert.Contains("CỘNG HÒA XÃ HỘI CHỦ NGHĨA VIỆT NAM", txt, StringComparison.Ordinal);
        AssertZipWith(await OkBytesAsync(await PostAsync("/convert", docx, "a.docx", ("to", "docx")), Docx), "word/document.xml");

        // .xls → .xlsx / .ods / .csv.
        var xls = await File.ReadAllBytesAsync(Fixture("sample.xls"));
        AssertZipWith(await OkBytesAsync(await PostAsync("/convert", xls, "Số liệu cũ.xls", ("to", "xlsx")), Xlsx), "xl/workbook.xml");
        AssertZipWith(await OkBytesAsync(await PostAsync("/convert", xls, "Số liệu cũ.xls", ("to", "ods")), "application/vnd.oasis.opendocument.spreadsheet"), "content.xml");
        Assert.NotEmpty(await OkBytesAsync(await PostAsync("/convert", xls, "Số liệu cũ.xls", ("to", "csv")), "text/csv"));

        var xlsx = await File.ReadAllBytesAsync(Corpus("sheet/values-1.xlsx"));
        AssertZipWith(await OkBytesAsync(await PostAsync("/convert", xlsx, "b.xlsx", ("to", "ods")), "application/vnd.oasis.opendocument.spreadsheet"), "content.xml");
        Assert.Contains("Chuỗi 1", System.Text.Encoding.UTF8.GetString(await OkBytesAsync(await PostAsync("/convert", xlsx, "b.xlsx", ("to", "csv")), "text/csv")), StringComparison.Ordinal);

        // .pptx → .pptx (normalised by the engine); .ppt → .odp / text (no PresentationML writer for it yet).
        var pptx = await File.ReadAllBytesAsync(Corpus("slide/slide-text-bullets.pptx"));
        AssertZipWith(await OkBytesAsync(await PostAsync("/convert", pptx, "c.pptx", ("to", "pptx")), Pptx), "ppt/presentation.xml");
        var ppt = await File.ReadAllBytesAsync(Fixture("sample.ppt"));
        AssertZipWith(await OkBytesAsync(await PostAsync("/convert", ppt, "Trình chiếu cũ.ppt", ("to", "odp")), "application/vnd.oasis.opendocument.presentation"), "content.xml");
        Assert.NotEmpty(await OkBytesAsync(await PostAsync("/convert", ppt, "Trình chiếu cũ.ppt", ("to", "txt")), "text/plain"));

        // Pairs bo-cli cannot do: 422 without running it.
        foreach (var (bytes, name, to) in new[] { (pptx, "c.pptx", "odp"), (ppt, "d.ppt", "pptx"), (xlsx, "b.xlsx", "docx"), (docx, "a.docx", "csv") })
        {
            var refused = await PostAsync("/convert", bytes, name, ("to", to));
            Assert.True(refused.StatusCode == HttpStatusCode.UnprocessableEntity, $"{name} → {to}: {(int)refused.StatusCode}");
        }
    }

    [Fact]
    public async Task PngPreviewRendersEveryPageWithTheRealCli()
    {
        if (_app is null)
        {
            return;
        }

        var c = _app.ClientFor("bo-cli-e2e");
        foreach (var (path, name) in new[] { ("doc/phase1-sample.docx", "a.docx"), ("sheet/values-1.xlsx", "b.xlsx"), ("slide/slide-text-bullets.pptx", "c.pptx") })
        {
            var res = await PostAsync("/preview", await File.ReadAllBytesAsync(Corpus(path)), name, ("format", "png"), ("scale", "0.3"));
            Assert.True(res.StatusCode == HttpStatusCode.OK, $"{name}: {(int)res.StatusCode} {await res.Content.ReadAsStringAsync()}");
            var body = await res.Content.ReadFromJsonAsync<JsonElement>();
            var pages = body.GetProperty("pages").GetInt32();
            Assert.InRange(pages, 1, 50);
            var images = body.GetProperty("images").EnumerateArray().Select(x => x.GetString()!).ToList();
            Assert.Equal(pages, images.Count);
            foreach (var url in images)
            {
                AssertPng(await c.GetByteArrayAsync(new Uri(url, UriKind.Relative)));
            }
        }
    }

    [Fact]
    public async Task ThumbnailsAndPreviewsOfEveryApp()
    {
        if (_app is null)
        {
            return;
        }

        foreach (var (path, name) in new[] { ("doc/phase1-sample.docx", "a.docx"), ("sheet/values-1.xlsx", "b.xlsx"), ("slide/slide-text-bullets.pptx", "c.pptx") })
        {
            var bytes = await File.ReadAllBytesAsync(Corpus(path));
            AssertPng(await OkBytesAsync(await PostAsync("/thumbnail", bytes, name, ("page", "1"), ("scale", "0.25")), "image/png"));
            AssertPdf(await OkBytesAsync(await PostAsync("/preview", bytes, name), "application/pdf"));
        }

        // Asynchronous form: 202 + poll + result.
        using var form = new MultipartFormDataContent { { new ByteArrayContent(await File.ReadAllBytesAsync(Corpus("doc/phase1-sample.docx"))), "file", "a.docx" } };
        var c = _app.ClientFor("bo-cli-e2e");
        var accepted = await c.PostAsync(new Uri("/thumbnail", UriKind.Relative), form);
        Assert.Equal(HttpStatusCode.Accepted, accepted.StatusCode);
        var id = (await accepted.Content.ReadFromJsonAsync<JsonElement>()).GetProperty("id").GetString();
        await CollabTests.Eventually(
            () => c.GetFromJsonAsync<JsonElement>(new Uri($"/jobs/{id}", UriKind.Relative)).GetAwaiter().GetResult().GetProperty("status").GetString() == "succeeded",
            60_000);
        AssertPng(await c.GetByteArrayAsync(new Uri($"/jobs/{id}/result", UriKind.Relative)));
    }

    [Fact]
    public async Task ExtractsTextForSearchIndexing()
    {
        if (_app is null)
        {
            return;
        }

        var doc = await (await PostAsync("/extract-text", await File.ReadAllBytesAsync(Corpus("doc/phase1-sample.docx")), "Công văn.docx")).Content.ReadFromJsonAsync<JsonElement>();
        Assert.Contains("CỘNG HÒA XÃ HỘI CHỦ NGHĨA VIỆT NAM", doc.GetProperty("text").GetString(), StringComparison.Ordinal);
        Assert.True(doc.GetProperty("pages").GetInt32() >= 1);
        Assert.Equal("document", doc.GetProperty("metadata").GetProperty("kind").GetString());

        var sheet = await (await PostAsync("/extract-text", await File.ReadAllBytesAsync(Corpus("sheet/values-1.xlsx")), "Số liệu.xlsx")).Content.ReadFromJsonAsync<JsonElement>();
        Assert.Contains("Chuỗi 1", sheet.GetProperty("text").GetString(), StringComparison.Ordinal);
        Assert.Equal("spreadsheet", sheet.GetProperty("metadata").GetProperty("kind").GetString());

        var deck = await (await PostAsync("/extract-text", await File.ReadAllBytesAsync(Corpus("slide/slide-text-bullets.pptx")), "Báo cáo.pptx")).Content.ReadFromJsonAsync<JsonElement>();
        Assert.Contains("Báo", deck.GetProperty("text").GetString(), StringComparison.Ordinal);
        Assert.Equal("presentation", deck.GetProperty("metadata").GetProperty("kind").GetString());
    }
}
