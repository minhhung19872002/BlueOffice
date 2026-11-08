using System.Net;
using System.Net.Http.Headers;
using System.Net.Http.Json;
using System.Text.Json;
using BlueOffice.Domain;
using BlueOffice.Infrastructure.Jobs;
using Xunit;

namespace BlueOffice.Tests;

/// <summary>Conversion services (prompt §6A.5) through the job queue and the worker, with a fake bo-cli.</summary>
public sealed class JobTests : IDisposable
{
    private readonly TestApp _app = new();

    public void Dispose() => _app.Dispose();

    private static MultipartFormDataContent Form(string fileName, params (string Key, string Value)[] fields)
    {
        var form = new MultipartFormDataContent();
        var file = new ByteArrayContent([0x50, 0x4B, 3, 4]);
        file.Headers.ContentType = new MediaTypeHeaderValue("application/octet-stream");
        form.Add(file, "file", fileName);
        foreach (var (k, v) in fields)
        {
            form.Add(new StringContent(v), k);
        }

        return form;
    }

    [Fact]
    public async Task ConvertWaitReturnsPdf()
    {
        var c = _app.ClientFor("job-user");
        using var form = Form("Tờ trình.docx", ("to", "pdf"));
        var res = await c.PostAsync(new Uri("/convert?wait=true", UriKind.Relative), form);
        Assert.Equal(HttpStatusCode.OK, res.StatusCode);
        Assert.Equal("application/pdf", res.Content.Headers.ContentType?.MediaType);
        Assert.StartsWith("%PDF", await res.Content.ReadAsStringAsync(), StringComparison.Ordinal);
        var call = Assert.Single(_app.Runner.Calls);
        Assert.Equal("convert", call[0]);
        Assert.EndsWith("input.docx", call[1], StringComparison.Ordinal); // client file name never reaches the command line
    }

    [Fact]
    public async Task AsyncJobCanBePolledAndOnlyByItsOwner()
    {
        var c = _app.ClientFor("job-owner");
        using var form = Form("slide.docx", ("page", "2"), ("scale", "0.5"));
        var res = await c.PostAsync(new Uri("/thumbnail", UriKind.Relative), form);
        Assert.Equal(HttpStatusCode.Accepted, res.StatusCode);
        var job = await res.Content.ReadFromJsonAsync<JsonElement>();
        var id = job.GetProperty("id").GetString();
        Assert.Equal($"/jobs/{id}", res.Headers.Location?.OriginalString);

        JsonElement status = default;
        await CollabTests.Eventually(() =>
        {
            status = c.GetFromJsonAsync<JsonElement>(new Uri($"/jobs/{id}", UriKind.Relative)).GetAwaiter().GetResult();
            return status.GetProperty("status").GetString() == "succeeded";
        });
        Assert.Equal($"/jobs/{id}/result", status.GetProperty("resultUrl").GetString());
        var png = await c.GetAsync(new Uri($"/jobs/{id}/result", UriKind.Relative));
        Assert.Equal("image/png", png.Content.Headers.ContentType?.MediaType);
        Assert.Equal(0x89, (await png.Content.ReadAsByteArrayAsync())[0]);
        var args = _app.Runner.Calls.Single(a => a[0] == "render");
        Assert.Equal(["--page", "2", "--scale", "0.5"], args.Skip(2).Take(4));

        Assert.Equal(HttpStatusCode.NotFound, (await _app.ClientFor("someone-else").GetAsync(new Uri($"/jobs/{id}", UriKind.Relative))).StatusCode);
        Assert.Equal(HttpStatusCode.Unauthorized, (await _app.CreateClient().GetAsync(new Uri($"/jobs/{id}", UriKind.Relative))).StatusCode);
    }

    [Fact]
    public async Task ExtractTextReadsEveryPage()
    {
        var c = _app.ClientFor("text-user");
        using var form = Form("báo cáo.docx");
        var res = await c.PostAsync(new Uri("/extract-text?wait=true", UriKind.Relative), form);
        Assert.Equal(HttpStatusCode.OK, res.StatusCode);
        var json = await res.Content.ReadFromJsonAsync<JsonElement>();
        Assert.Equal("Xin chào thế giới\nDòng hai\n\f\nTrang 2", json.GetProperty("text").GetString());
        Assert.Equal(2, json.GetProperty("pages").GetInt32());
        Assert.Equal("báo cáo.docx", json.GetProperty("metadata").GetProperty("fileName").GetString());
    }

    [Fact]
    public async Task PreviewProducesPdf()
    {
        using var form = Form("bang.md");
        var res = await _app.ClientFor("preview-user").PostAsync(new Uri("/preview?wait=true", UriKind.Relative), form);
        Assert.Equal(HttpStatusCode.OK, res.StatusCode);
        Assert.Equal("application/pdf", res.Content.Headers.ContentType?.MediaType);
    }

    [Fact]
    public async Task PngPreviewStoresOneImagePerPageForTheOwnerOnly()
    {
        _app.Runner.RenderPages = ConversionJobProcessor.MaxPreviewPages + 3;
        var c = _app.ClientFor("preview-png");
        using var form = Form("Báo cáo.xlsx", ("format", "png"));
        var res = await c.PostAsync(new Uri("/preview?wait=true", UriKind.Relative), form);
        Assert.Equal(HttpStatusCode.OK, res.StatusCode);
        Assert.Equal("application/json", res.Content.Headers.ContentType?.MediaType);
        var body = await res.Content.ReadFromJsonAsync<JsonElement>();
        Assert.Equal(ConversionJobProcessor.MaxPreviewPages, body.GetProperty("pages").GetInt32());
        Assert.Equal(ConversionJobProcessor.MaxPreviewPages + 3, body.GetProperty("totalPages").GetInt32());
        var images = body.GetProperty("images").EnumerateArray().Select(x => x.GetString()!).ToList();
        Assert.Equal(ConversionJobProcessor.MaxPreviewPages, images.Count);

        // One render call for every page at 1 px/pt by default.
        var args = _app.Runner.Calls.Single(a => a[0] == "render");
        Assert.EndsWith("page-%d.png", args[3], StringComparison.Ordinal);
        Assert.Equal(["--scale", "1"], args.Skip(4).Take(2));

        var second = await c.GetAsync(new Uri(images[1], UriKind.Relative));
        Assert.Equal(HttpStatusCode.OK, second.StatusCode);
        Assert.Equal("image/png", second.Content.Headers.ContentType?.MediaType);
        Assert.Equal(2, (await second.Content.ReadAsByteArrayAsync())[^1]);
        var id = images[0].Split('/')[2];
        Assert.Equal(HttpStatusCode.NotFound, (await c.GetAsync(new Uri($"/jobs/{id}/pages/{ConversionJobProcessor.MaxPreviewPages + 1}", UriKind.Relative))).StatusCode);
        Assert.Equal(HttpStatusCode.NotFound, (await c.GetAsync(new Uri($"/jobs/{id}/pages/0", UriKind.Relative))).StatusCode);
        Assert.Equal(HttpStatusCode.NotFound, (await _app.ClientFor("someone-else").GetAsync(new Uri(images[0], UriKind.Relative))).StatusCode);
    }

    [Fact]
    public async Task PreviewRejectsUnknownFormatsAndUnrenderableInputs()
    {
        var c = _app.ClientFor("preview-bad");
        using (var form = Form("a.docx", ("format", "gif")))
        {
            Assert.Equal(HttpStatusCode.BadRequest, (await c.PostAsync(new Uri("/preview?wait=true", UriKind.Relative), form)).StatusCode);
        }

        using (var form = Form("a.ppt", ("format", "png")))
        {
            Assert.Equal(HttpStatusCode.UnprocessableEntity, (await c.PostAsync(new Uri("/preview?wait=true", UriKind.Relative), form)).StatusCode);
        }
    }

    [Fact]
    public async Task MacroEnabledPackagesKeepTheirExtensionAndType()
    {
        _app.Runner.Handler = args =>
        {
            if (args[0] != "convert")
            {
                return null;
            }

            // A .doc with VBA comes back as a macro-enabled package.
            using var zip = System.IO.Compression.ZipFile.Open(args[^1], System.IO.Compression.ZipArchiveMode.Create);
            using (var w = new StreamWriter(zip.CreateEntry("[Content_Types].xml").Open()))
            {
                w.Write("<Types><Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.ms-word.document.macroEnabled.main+xml\"/></Types>");
            }

            return new ProcessResult(0, "", "note: the input contains macros");
        };
        using var form = Form("Biểu mẫu.doc", ("to", "docx"));
        var res = await _app.ClientFor("macro-user").PostAsync(new Uri("/convert?wait=true", UriKind.Relative), form);
        Assert.Equal(HttpStatusCode.OK, res.StatusCode);
        Assert.Equal("application/vnd.ms-word.document.macroEnabled.12", res.Content.Headers.ContentType?.MediaType);
        Assert.EndsWith(".docm", res.Content.Headers.ContentDisposition?.FileNameStar ?? res.Content.Headers.ContentDisposition?.FileName?.Trim('"'), StringComparison.Ordinal);
    }

    [Fact]
    public async Task FailuresAreReported()
    {
        var c = _app.ClientFor("fail-user");
        using (var form = Form("virus.exe"))
        {
            var res = await c.PostAsync(new Uri("/convert?wait=true", UriKind.Relative), form);
            Assert.Equal(HttpStatusCode.UnprocessableEntity, res.StatusCode);
            Assert.Contains("Unsupported input type", await res.Content.ReadAsStringAsync(), StringComparison.Ordinal);
        }

        using (var form = Form("a.docx", ("to", "xlsx")))
        {
            Assert.Equal(HttpStatusCode.UnprocessableEntity, (await c.PostAsync(new Uri("/convert?wait=true", UriKind.Relative), form)).StatusCode);
        }

        _app.Runner.Handler = a => a[0] == "convert" ? new ProcessResult(2, "", "error: broken file") : null;
        using (var form = Form("broken.docx"))
        {
            var res = await c.PostAsync(new Uri("/convert?wait=true", UriKind.Relative), form);
            Assert.Equal(HttpStatusCode.UnprocessableEntity, res.StatusCode);
            Assert.Contains("broken file", await res.Content.ReadAsStringAsync(), StringComparison.Ordinal);
        }

        Assert.Equal(HttpStatusCode.BadRequest, (await c.PostAsync(new Uri("/convert", UriKind.Relative), new ByteArrayContent([1]))).StatusCode);
    }

    [Fact]
    public void TextExtractionIgnoresMalformedCommands()
    {
        Assert.Equal("", TextExtraction.FromDisplayList("{}"));
        Assert.Equal("ab\nc", TextExtraction.FromDisplayList("""{"commands":[1,{"op":"glyphs","text":"a","y":1},{"op":"glyphs","text":"b"},{"op":"glyphs","text":3},{"op":"glyphs","text":"c","y":9}]}"""));
    }
}

public sealed class HostServiceJobTests : IAsyncLifetime
{
    private MockDrive _drive = null!;
    private TestApp _app = null!;

    public async Task InitializeAsync()
    {
        _drive = await MockDrive.StartAsync();
        _app = new TestApp(_drive);
    }

    public async Task DisposeAsync()
    {
        _app.Dispose();
        await _drive.DisposeAsync();
    }

    [Fact]
    public async Task BlueDriveServiceTokenCanUseConversionServices()
    {
        var c = _app.CreateClient();
        c.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Bearer", await _drive.TokenAsync("bluedrive-service"));
        using var form = new MultipartFormDataContent { { new ByteArrayContent([1]), "file", "a.docx" } };
        var res = await c.PostAsync(new Uri("/thumbnail?wait=true", UriKind.Relative), form);
        Assert.Equal(HttpStatusCode.OK, res.StatusCode);
        Assert.Equal("image/png", res.Content.Headers.ContentType?.MediaType);

        // Host tokens do not grant access to the user file API.
        Assert.Equal(HttpStatusCode.Unauthorized, (await c.GetAsync(new Uri("/api/folders", UriKind.Relative))).StatusCode);
    }
}
