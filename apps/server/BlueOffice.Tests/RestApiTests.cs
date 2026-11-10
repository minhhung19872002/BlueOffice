using System.IO.Compression;
using System.Net;
using System.Net.Http.Headers;
using System.Net.Http.Json;
using System.Text;
using System.Text.Json;
using BlueOffice.Api.Rest;
using BlueOffice.Domain;
using Microsoft.AspNetCore.Routing;
using Microsoft.Extensions.DependencyInjection;
using Xunit;

namespace BlueOffice.Tests;

/// <summary>Shared helpers of the REST API tests (CORE-API-001).</summary>
internal static class RestApi
{
    /// <summary>Key with every scope.</summary>
    public const string FullKey = "erp-key-0123456789abcdef0123456789abcdef";

    /// <summary>Key limited to <c>inspect</c>.</summary>
    public const string InspectKey = "reader-key-0123456789abcdef0123456789abc";

    /// <summary>Key limited to <c>convert</c>.</summary>
    public const string ConvertKey = "convert-key-0123456789abcdef0123456789ab";

    /// <summary>Revoked key.</summary>
    public const string RevokedKey = "revoked-key-0123456789abcdef0123456789ab";

    /// <summary>Settings enabling the API with the keys above; <paramref name="extra"/> overrides.</summary>
    public static Dictionary<string, string?> Settings(params (string Key, string? Value)[] extra)
    {
        var s = new Dictionary<string, string?>
        {
            ["RestApi:Enabled"] = "true",
            ["RestApi:MaxUploadBytes"] = "4096",
            ["RestApi:Keys:0:Name"] = "erp",
            ["RestApi:Keys:0:Sha256"] = ApiKeyAuth.Hash(FullKey),
            ["RestApi:Keys:1:Name"] = "reader",
            ["RestApi:Keys:1:Sha256"] = ApiKeyAuth.Hash(InspectKey),
            ["RestApi:Keys:1:Scopes:0"] = "inspect",
            ["RestApi:Keys:2:Name"] = "converter",
            ["RestApi:Keys:2:Sha256"] = ApiKeyAuth.Hash(ConvertKey).ToUpperInvariant(),
            ["RestApi:Keys:2:Scopes:0"] = "convert",
            ["RestApi:Keys:3:Name"] = "old",
            ["RestApi:Keys:3:Sha256"] = ApiKeyAuth.Hash(RevokedKey),
            ["RestApi:Keys:3:Enabled"] = "false",
        };
        foreach (var (k, v) in extra)
        {
            s[k] = v;
        }

        return s;
    }

    /// <summary>Client sending <paramref name="key"/> as <c>X-Api-Key</c> (none when null).</summary>
    public static HttpClient WithKey(this TestApp app, string? key)
    {
        var c = app.CreateClient();
        if (key is not null)
        {
            c.DefaultRequestHeaders.Add(ApiKeyAuth.Header, key);
        }

        return c;
    }

    /// <summary>Multipart form: files (field, name, bytes) and values.</summary>
    public static MultipartFormDataContent Form((string Field, string Name, byte[] Content)[] files, params (string Key, string Value)[] fields)
    {
        var form = new MultipartFormDataContent();
        foreach (var (field, name, content) in files)
        {
            var part = new ByteArrayContent(content);
            part.Headers.ContentType = new MediaTypeHeaderValue("application/octet-stream");
            form.Add(part, field, name);
        }

        foreach (var (k, v) in fields)
        {
            form.Add(new StringContent(v, Encoding.UTF8), k);
        }

        return form;
    }

    /// <summary>A form with one file part <c>file</c>.</summary>
    public static MultipartFormDataContent FileForm(string name, params (string Key, string Value)[] fields) =>
        Form([("file", name, [0x50, 0x4B, 3, 4])], fields);

    /// <summary>POSTs to <c>/api/v1/documents/{operation}</c>.</summary>
    public static async Task<HttpResponseMessage> PostDocAsync(this HttpClient c, string operation, MultipartFormDataContent form)
    {
        using (form)
        {
            return await c.PostAsync(new Uri($"/api/v1/documents/{operation}", UriKind.Relative), form);
        }
    }

    /// <summary>Status of a request whose body does not matter.</summary>
    public static async Task<HttpStatusCode> StatusAsync(this HttpClient c, string operation, MultipartFormDataContent form) =>
        (await c.PostDocAsync(operation, form)).StatusCode;

    /// <summary>Problem detail text of an error response.</summary>
    public static async Task<string> DetailAsync(this HttpResponseMessage res) =>
        (await res.Content.ReadFromJsonAsync<JsonElement>()).GetProperty("detail").GetString() ?? "";
}

/// <summary>
/// Document REST API (<c>/api/v1</c>, CORE-API-001) with a fake bo-cli: authentication by API key and scopes, input
/// validation, size limit, the command line handed to the worker, asynchronous jobs, rate limit and the OpenAPI document.
/// </summary>
public sealed class RestApiTests : IDisposable
{
    private readonly TestApp _app = new(settings: RestApi.Settings());

    public void Dispose() => _app.Dispose();

    private IReadOnlyList<string> Call(string command) => _app.Runner.Calls.Single(a => a[0] == command);

    [Fact]
    public async Task ApiIsOffByDefault()
    {
        using var app = new TestApp();
        Assert.Equal(HttpStatusCode.NotFound, await app.ClientFor("u").StatusAsync("convert", RestApi.FileForm("a.docx")));
        Assert.Equal(HttpStatusCode.NotFound, (await app.CreateClient().GetAsync(new Uri("/api/v1/openapi.json", UriKind.Relative))).StatusCode);
    }

    [Fact]
    public async Task ConvertReturnsPdfWithPdfOptions()
    {
        var res = await _app.WithKey(RestApi.FullKey).PostDocAsync("convert", RestApi.FileForm("Tờ trình.docx", ("to", "pdf"), ("pdfa", "2B"), ("tagged", "true"), ("bookmarks", "1")));
        Assert.Equal(HttpStatusCode.OK, res.StatusCode);
        Assert.Equal("application/pdf", res.Content.Headers.ContentType?.MediaType);
        Assert.StartsWith("%PDF", await res.Content.ReadAsStringAsync(), StringComparison.Ordinal);
        var call = Call("convert");
        Assert.EndsWith("input.docx", call[1], StringComparison.Ordinal); // the client file name never reaches the command line
        Assert.Equal(["--pdfa", "2b", "--tagged", "--bookmarks"], call.Skip(6));

        // Default target: pdf, no options.
        _app.Runner.Calls.Clear();
        Assert.Equal(HttpStatusCode.OK, await _app.WithKey(RestApi.ConvertKey).StatusAsync("convert", RestApi.FileForm("bảng.xlsx")));
        Assert.Equal(["--to", "pdf"], Call("convert").Skip(2).Take(2));
        Assert.Equal(6, Call("convert").Count);

        // Bearer tokens of the Service policy are accepted too.
        Assert.Equal(HttpStatusCode.OK, await _app.ClientFor("rest-user").StatusAsync("convert", RestApi.FileForm("a.docx", ("to", "odt"))));
    }

    [Fact]
    public async Task ConvertRejectsBadCredentials()
    {
        Assert.Equal(HttpStatusCode.Unauthorized, await _app.WithKey(null).StatusAsync("convert", RestApi.FileForm("a.docx")));
        Assert.Equal(HttpStatusCode.Unauthorized, await _app.WithKey("not-a-key").StatusAsync("convert", RestApi.FileForm("a.docx")));
        Assert.Equal(HttpStatusCode.Unauthorized, await _app.WithKey(RestApi.RevokedKey).StatusAsync("convert", RestApi.FileForm("a.docx")));

        // The hash of a key is not a key.
        Assert.Equal(HttpStatusCode.Unauthorized, await _app.WithKey(ApiKeyAuth.Hash(RestApi.FullKey)).StatusAsync("convert", RestApi.FileForm("a.docx")));

        var forbidden = await _app.WithKey(RestApi.InspectKey).PostDocAsync("convert", RestApi.FileForm("a.docx"));
        Assert.Equal(HttpStatusCode.Forbidden, forbidden.StatusCode);
        Assert.Contains("'convert'", await forbidden.DetailAsync(), StringComparison.Ordinal);
        Assert.Empty(_app.Runner.Calls);
    }

    [Fact]
    public async Task ConvertRejectsBadInput()
    {
        var c = _app.WithKey(RestApi.FullKey);
        Assert.Equal(HttpStatusCode.BadRequest, await c.StatusAsync("convert", RestApi.Form([], ("to", "pdf"))));
        Assert.Equal(HttpStatusCode.BadRequest, await c.StatusAsync("convert", RestApi.Form([("file", "a.docx", [])])));
        Assert.Equal(HttpStatusCode.BadRequest, await c.StatusAsync("convert", RestApi.FileForm("virus.exe")));
        var target = await c.PostDocAsync("convert", RestApi.FileForm("a.docx", ("to", "xlsx")));
        Assert.Equal(HttpStatusCode.BadRequest, target.StatusCode);
        Assert.Contains("pdf, docx, odt, txt", await target.DetailAsync(), StringComparison.Ordinal);
        Assert.Equal(HttpStatusCode.BadRequest, await c.StatusAsync("convert", RestApi.FileForm("a.docx", ("pdfa", "9z"))));
        Assert.Equal(HttpStatusCode.BadRequest, await c.StatusAsync("convert", RestApi.FileForm("a.docx", ("to", "odt"), ("tagged", "true"))));
        using var notForm = new ByteArrayContent([1]);
        Assert.Equal(HttpStatusCode.BadRequest, (await c.PostAsync(new Uri("/api/v1/documents/convert", UriKind.Relative), notForm)).StatusCode);
        Assert.Empty(_app.Runner.Calls);

        // A file bo-cli cannot read: 422 with its message.
        _app.Runner.Handler = a => a[0] == "convert" ? new ProcessResult(2, "", "error: broken file") : null;
        var broken = await c.PostDocAsync("convert", RestApi.FileForm("broken.docx"));
        Assert.Equal(HttpStatusCode.UnprocessableEntity, broken.StatusCode);
        Assert.Contains("broken file", await broken.DetailAsync(), StringComparison.Ordinal);
    }

    [Fact]
    public async Task ConvertEnforcesTheSizeLimit()
    {
        var res = await _app.WithKey(RestApi.FullKey).PostDocAsync("convert", RestApi.Form([("file", "big.docx", new byte[5000])]));
        Assert.Equal(HttpStatusCode.RequestEntityTooLarge, res.StatusCode);
        Assert.Contains("4096", await res.DetailAsync(), StringComparison.Ordinal);
        Assert.Empty(_app.Runner.Calls);
    }

    [Fact]
    public async Task MergeProducesOneDocumentOrOneFilePerRecord()
    {
        var c = _app.WithKey(RestApi.FullKey);
        string? dataSeen = null;
        _app.Runner.Handler = a =>
        {
            if (a[0] == "merge")
            {
                dataSeen = File.ReadAllText(a[2]);
            }

            return null;
        };
        const string Csv = "HoTen,DonVi\nNguyễn Văn An,Sở Kế hoạch\n";
        var single = await c.PostDocAsync("merge", RestApi.Form([("template", "Thư mời.docx", [1, 2]), ("data", "../../danh sách.CSV", Encoding.UTF8.GetBytes(Csv))], ("to", "pdf")));
        Assert.Equal(HttpStatusCode.OK, single.StatusCode);
        Assert.Equal("application/pdf", single.Content.Headers.ContentType?.MediaType);
        var call = Call("merge");
        Assert.EndsWith("input.docx", call[1], StringComparison.Ordinal);
        Assert.Equal("data.csv", Path.GetFileName(call[2])); // fixed name: the client's path is ignored
        Assert.Equal(["--to", "pdf", "--out"], call.Skip(3).Take(3));
        Assert.Equal(7, call.Count);
        Assert.Equal(Csv, dataSeen);

        // One file per record: records as JSON, files named by a pattern.
        _app.Runner.Calls.Clear();
        const string Records = """[{"HoTen":"Nguyễn Văn An"},{"HoTen":"Trần Thị Bình"}]""";
        var zip = await c.PostDocAsync("merge", RestApi.Form([("template", "Thư mời.docx", [1, 2])], ("records", Records), ("output", "zip"), ("to", "docx"), ("pattern", "--Thư «HoTen»")));
        Assert.Equal(HttpStatusCode.OK, zip.StatusCode);
        Assert.Equal("application/zip", zip.Content.Headers.ContentType?.MediaType);
        Assert.StartsWith("PK", await zip.Content.ReadAsStringAsync(), StringComparison.Ordinal);
        call = Call("merge");
        Assert.Equal("data.json", Path.GetFileName(call[2]));
        Assert.Equal(["--to", "zip"], call.Skip(3).Take(2));
        Assert.Equal(["--each", "docx", "--pattern=--Thư «HoTen»"], call.Skip(7));
        Assert.Equal(Records, dataSeen);
    }

    [Fact]
    public async Task MergeRejectsBadCredentialsBadInputAndLargeFiles()
    {
        (string, string, byte[]) template = ("template", "t.docx", [1]);
        (string, string, byte[]) data = ("data", "d.csv", "a\n1\n"u8.ToArray());
        Assert.Equal(HttpStatusCode.Unauthorized, await _app.WithKey(null).StatusAsync("merge", RestApi.Form([template, data])));
        Assert.Equal(HttpStatusCode.Forbidden, await _app.WithKey(RestApi.ConvertKey).StatusAsync("merge", RestApi.Form([template, data])));

        var c = _app.WithKey(RestApi.FullKey);
        Assert.Equal(HttpStatusCode.BadRequest, await c.StatusAsync("merge", RestApi.Form([data])));
        Assert.Equal(HttpStatusCode.BadRequest, await c.StatusAsync("merge", RestApi.Form([template])));
        Assert.Equal(HttpStatusCode.BadRequest, await c.StatusAsync("merge", RestApi.Form([template, data], ("records", "[{}]"))));
        Assert.Equal(HttpStatusCode.BadRequest, await c.StatusAsync("merge", RestApi.Form([("template", "t.xlsx", [1]), data])));
        Assert.Equal(HttpStatusCode.BadRequest, await c.StatusAsync("merge", RestApi.Form([template, ("data", "d.exe", [1])])));
        Assert.Equal(HttpStatusCode.BadRequest, await c.StatusAsync("merge", RestApi.Form([template, data], ("to", "odt"))));
        Assert.Equal(HttpStatusCode.BadRequest, await c.StatusAsync("merge", RestApi.Form([template, data], ("output", "each"))));
        Assert.Equal(HttpStatusCode.BadRequest, await c.StatusAsync("merge", RestApi.Form([template, data], ("pattern", new string('a', 201)))));
        foreach (var records in new[] { "{\"a\":1}", "[1,2]", "[]", "not json" })
        {
            Assert.Equal(HttpStatusCode.BadRequest, await c.StatusAsync("merge", RestApi.Form([template], ("records", records))));
        }

        Assert.Equal(HttpStatusCode.RequestEntityTooLarge, await c.StatusAsync("merge", RestApi.Form([template, ("data", "d.csv", new byte[5000])])));
        Assert.Equal(HttpStatusCode.RequestEntityTooLarge, await c.StatusAsync("merge", RestApi.Form([("template", "t.docx", new byte[5000]), data])));
        Assert.Empty(_app.Runner.Calls);

        using var limited = new TestApp(settings: RestApi.Settings(("RestApi:MaxRecords", "2")));
        var many = await limited.WithKey(RestApi.FullKey).PostDocAsync("merge", RestApi.Form([template], ("records", "[{},{},{}]")));
        Assert.Equal(HttpStatusCode.BadRequest, many.StatusCode);
        Assert.Contains("at most 2", await many.DetailAsync(), StringComparison.Ordinal);
    }

    [Fact]
    public async Task EditFillsATemplateAndReportsTheEdits()
    {
        var c = _app.WithKey(RestApi.FullKey);
        string? editsSeen = null;
        _app.Runner.Handler = a =>
        {
            if (a[0] == "edit")
            {
                editsSeen = File.ReadAllText(a[3]);
            }

            return null;
        };
        const string Edits = """[{"op":"setControl","tag":"ho_ten","value":"Nguyễn Văn An"},{"op":"setBookmark","name":"KhongCo","text":"x"}]""";
        var res = await c.PostDocAsync("edit", RestApi.FileForm("Hợp đồng.docx", ("edits", Edits), ("to", "pdf"), ("strict", "false")));
        Assert.Equal(HttpStatusCode.OK, res.StatusCode);
        Assert.Equal("application/pdf", res.Content.Headers.ContentType?.MediaType);
        Assert.Equal("1", res.Headers.GetValues("X-Edits-Applied").Single());
        Assert.Equal("1", res.Headers.GetValues("X-Edits-Unmatched").Single());
        var call = Call("edit");
        Assert.EndsWith("input.docx", call[1], StringComparison.Ordinal);
        Assert.Equal("edits.json", Path.GetFileName(call[3]));
        Assert.Equal(["--to", "pdf"], call.Skip(6));
        Assert.Equal(Edits, editsSeen);

        // Strict by default; edits may also come as a file part; a workbook keeps its format.
        _app.Runner.Calls.Clear();
        const string Cells = """[{"op":"setCell","ref":"TyGia","value":25000}]""";
        Assert.Equal(HttpStatusCode.OK, await c.StatusAsync("edit", RestApi.Form([("file", "Báo cáo.xlsx", [1]), ("edits", "edits.json", Encoding.UTF8.GetBytes(Cells))])));
        Assert.Equal(["--to", "xlsx", "--strict"], Call("edit").Skip(6));
        Assert.Equal(Cells, editsSeen);

        // bo-cli refusing (strict: an edit matched nothing) is a 422 with its message.
        _app.Runner.Handler = a => a[0] == "edit" ? new ProcessResult(1, "", "error: edit matched nothing: setBookmark 'KhongCo'") : null;
        var unmatched = await c.PostDocAsync("edit", RestApi.FileForm("a.docx", ("edits", Edits)));
        Assert.Equal(HttpStatusCode.UnprocessableEntity, unmatched.StatusCode);
        Assert.Contains("matched nothing", await unmatched.DetailAsync(), StringComparison.Ordinal);
    }

    [Fact]
    public async Task EditRejectsBadCredentialsBadInputAndLargeFiles()
    {
        const string Edits = """[{"op":"replaceText","find":"{{a}}","with":"b"}]""";
        Assert.Equal(HttpStatusCode.Unauthorized, await _app.WithKey("wrong").StatusAsync("edit", RestApi.FileForm("a.docx", ("edits", Edits))));
        Assert.Equal(HttpStatusCode.Forbidden, await _app.WithKey(RestApi.InspectKey).StatusAsync("edit", RestApi.FileForm("a.docx", ("edits", Edits))));

        var c = _app.WithKey(RestApi.FullKey);
        Assert.Equal(HttpStatusCode.BadRequest, await c.StatusAsync("edit", RestApi.Form([], ("edits", Edits))));
        Assert.Equal(HttpStatusCode.BadRequest, await c.StatusAsync("edit", RestApi.FileForm("a.docx")));
        Assert.Equal(HttpStatusCode.BadRequest, await c.StatusAsync("edit", RestApi.FileForm("a.pptx", ("edits", Edits))));
        Assert.Equal(HttpStatusCode.BadRequest, await c.StatusAsync("edit", RestApi.FileForm("a.docx", ("edits", Edits), ("to", "xlsx"))));
        foreach (var bad in new[] { "{", "{\"op\":\"setCell\"}", "[]", "[1]", "[{\"find\":\"a\"}]", "[{\"op\":\"runMacro\"}]" })
        {
            var res = await c.PostDocAsync("edit", RestApi.FileForm("a.docx", ("edits", bad)));
            Assert.True(res.StatusCode == HttpStatusCode.BadRequest, bad);
        }

        Assert.Equal(HttpStatusCode.RequestEntityTooLarge, await c.StatusAsync("edit", RestApi.Form([("file", "a.docx", new byte[5000])], ("edits", Edits))));
        Assert.Empty(_app.Runner.Calls);

        using var limited = new TestApp(settings: RestApi.Settings(("RestApi:MaxEdits", "1")));
        var many = await limited.WithKey(RestApi.FullKey).PostDocAsync("edit", RestApi.FileForm("a.docx", ("edits", $"[{Edits[1..^1]},{Edits[1..^1]}]")));
        Assert.Equal(HttpStatusCode.BadRequest, many.StatusCode);
        Assert.Contains("at most 1", await many.DetailAsync(), StringComparison.Ordinal);
    }

    [Fact]
    public async Task InspectReportsFormatVariantEncryptionAndMacros()
    {
        // Any extension: the format is detected from the content.
        var res = await _app.WithKey(RestApi.InspectKey).PostDocAsync("inspect", RestApi.FileForm("Biểu mẫu.bin"));
        Assert.Equal(HttpStatusCode.OK, res.StatusCode);
        Assert.Equal("application/json", res.Content.Headers.ContentType?.MediaType);
        var json = await res.Content.ReadFromJsonAsync<JsonElement>();
        Assert.Equal("Biểu mẫu.bin", json.GetProperty("fileName").GetString());
        Assert.Equal("docx", json.GetProperty("format").GetString());
        Assert.Equal("docm", json.GetProperty("variant").GetString());
        Assert.False(json.GetProperty("encrypted").GetBoolean());
        Assert.True(json.GetProperty("macros").GetBoolean());
        var call = Call("inspect");
        Assert.Equal("input.bin", Path.GetFileName(call[1]));
        Assert.Equal("--json", call[2]);
    }

    [Fact]
    public async Task InspectRejectsBadCredentialsBadInputAndLargeFiles()
    {
        Assert.Equal(HttpStatusCode.Unauthorized, await _app.WithKey(null).StatusAsync("inspect", RestApi.FileForm("a.docx")));
        Assert.Equal(HttpStatusCode.Forbidden, await _app.WithKey(RestApi.ConvertKey).StatusAsync("inspect", RestApi.FileForm("a.docx")));
        var c = _app.WithKey(RestApi.FullKey);
        Assert.Equal(HttpStatusCode.BadRequest, await c.StatusAsync("inspect", RestApi.Form([], ("x", "y"))));
        Assert.Equal(HttpStatusCode.BadRequest, await c.StatusAsync("inspect", RestApi.Form([("file", "a.docx", [])])));
        Assert.Equal(HttpStatusCode.RequestEntityTooLarge, await c.StatusAsync("inspect", RestApi.Form([("file", "a.docx", new byte[5000])])));
        Assert.Empty(_app.Runner.Calls);

        _app.Runner.Handler = a => a[0] == "inspect" ? new ProcessResult(1, "", "error: not an Office document") : null;
        var junk = await c.PostDocAsync("inspect", RestApi.FileForm("junk.docx"));
        Assert.Equal(HttpStatusCode.UnprocessableEntity, junk.StatusCode);
        Assert.Contains("not an Office document", await junk.DetailAsync(), StringComparison.Ordinal);
    }

    [Fact]
    public async Task AsyncJobIsPolledUnderTheVersionedPrefixByItsOwnerOnly()
    {
        var c = _app.WithKey(RestApi.FullKey);
        using var form = RestApi.Form([("template", "t.docx", [1])], ("records", "[{\"a\":\"1\"}]"), ("output", "zip"));
        var res = await c.PostAsync(new Uri("/api/v1/documents/merge?wait=false", UriKind.Relative), form);
        Assert.Equal(HttpStatusCode.Accepted, res.StatusCode);
        var job = await res.Content.ReadFromJsonAsync<JsonElement>();
        var id = job.GetProperty("id").GetString();
        Assert.Equal("merge", job.GetProperty("kind").GetString());
        Assert.Equal($"/api/v1/jobs/{id}", res.Headers.Location?.OriginalString);
        Assert.Equal($"/api/v1/jobs/{id}", job.GetProperty("statusUrl").GetString());

        JsonElement status = default;
        await CollabTests.Eventually(() =>
        {
            status = c.GetFromJsonAsync<JsonElement>(new Uri($"/api/v1/jobs/{id}", UriKind.Relative)).GetAwaiter().GetResult();
            return status.GetProperty("status").GetString() == "succeeded";
        });
        Assert.Equal($"/api/v1/jobs/{id}/result", status.GetProperty("resultUrl").GetString());
        var result = await c.GetAsync(new Uri($"/api/v1/jobs/{id}/result", UriKind.Relative));
        Assert.Equal("application/zip", result.Content.Headers.ContentType?.MediaType);

        // Another key, a user with the same name as the key, and anonymous callers do not see the job.
        Assert.Equal(HttpStatusCode.NotFound, (await _app.WithKey(RestApi.InspectKey).GetAsync(new Uri($"/api/v1/jobs/{id}", UriKind.Relative))).StatusCode);
        Assert.Equal(HttpStatusCode.NotFound, (await _app.ClientFor("erp").GetAsync(new Uri($"/api/v1/jobs/{id}/result", UriKind.Relative))).StatusCode);
        Assert.Equal(HttpStatusCode.Unauthorized, (await _app.CreateClient().GetAsync(new Uri($"/api/v1/jobs/{id}", UriKind.Relative))).StatusCode);
        Assert.Equal(HttpStatusCode.NotFound, (await c.GetAsync(new Uri("/api/v1/jobs/unknown", UriKind.Relative))).StatusCode);
    }

    [Fact]
    public async Task ApiKeysOnlyOpenTheRestApi()
    {
        var c = _app.WithKey(RestApi.FullKey);
        Assert.Equal(HttpStatusCode.Unauthorized, (await c.GetAsync(new Uri("/api/folders", UriKind.Relative))).StatusCode);
        using var form = RestApi.FileForm("a.docx");
        Assert.Equal(HttpStatusCode.Unauthorized, (await c.PostAsync(new Uri("/convert?wait=true", UriKind.Relative), form)).StatusCode);
    }

    [Fact]
    public async Task BearerTokensCanBeTurnedOff()
    {
        using var app = new TestApp(settings: RestApi.Settings(("RestApi:AllowBearer", "false")));
        Assert.Equal(HttpStatusCode.Unauthorized, await app.ClientFor("rest-user").StatusAsync("inspect", RestApi.FileForm("a.docx")));
        Assert.Equal(HttpStatusCode.OK, await app.WithKey(RestApi.FullKey).StatusAsync("inspect", RestApi.FileForm("a.docx")));
    }

    [Fact]
    public async Task ExpiredKeysAreRefused()
    {
        var clock = new TestClock();
        using var app = new TestApp(clock: clock, settings: RestApi.Settings(("RestApi:Keys:0:ExpiresAt", clock.GetUtcNow().AddHours(1).ToString("O"))));
        Assert.Equal(HttpStatusCode.OK, await app.WithKey(RestApi.FullKey).StatusAsync("inspect", RestApi.FileForm("a.docx")));
        clock.Advance(TimeSpan.FromHours(2));
        Assert.Equal(HttpStatusCode.Unauthorized, await app.WithKey(RestApi.FullKey).StatusAsync("inspect", RestApi.FileForm("a.docx")));
    }

    [Fact]
    public async Task RequestsAreRateLimitedPerCaller()
    {
        using var app = new TestApp(settings: RestApi.Settings(("RestApi:RateLimit:PermitLimit", "2"), ("RestApi:RateLimit:Window", "00:10:00")));
        var c = app.WithKey(RestApi.FullKey);
        Assert.Equal(HttpStatusCode.OK, await c.StatusAsync("inspect", RestApi.FileForm("a.docx")));
        Assert.Equal(HttpStatusCode.BadRequest, await c.StatusAsync("inspect", RestApi.Form([], ("x", "y")))); // refused requests count too
        var limited = await c.PostDocAsync("inspect", RestApi.FileForm("a.docx"));
        Assert.Equal(HttpStatusCode.TooManyRequests, limited.StatusCode);
        Assert.True(int.Parse(limited.Headers.GetValues("Retry-After").Single(), System.Globalization.CultureInfo.InvariantCulture) > 0);
        Assert.Equal(429, (await limited.Content.ReadFromJsonAsync<JsonElement>()).GetProperty("status").GetInt32());

        // Each caller has its own window.
        Assert.Equal(HttpStatusCode.OK, await app.WithKey(RestApi.InspectKey).StatusAsync("inspect", RestApi.FileForm("a.docx")));
        Assert.Equal(HttpStatusCode.OK, await app.ClientFor("rest-user").StatusAsync("inspect", RestApi.FileForm("a.docx")));
    }

    [Fact]
    public async Task OpenApiDocumentDescribesEveryRoute()
    {
        var res = await _app.CreateClient().GetAsync(new Uri("/api/v1/openapi.json", UriKind.Relative));
        Assert.Equal(HttpStatusCode.OK, res.StatusCode);
        var doc = await res.Content.ReadFromJsonAsync<JsonElement>();
        Assert.StartsWith("3.0", doc.GetProperty("openapi").GetString(), StringComparison.Ordinal);
        var paths = doc.GetProperty("paths");
        var routes = _app.Services.GetRequiredService<EndpointDataSource>().Endpoints.OfType<RouteEndpoint>()
            .Where(e => e.RoutePattern.RawText!.StartsWith("/api/v1/", StringComparison.Ordinal) && !e.RoutePattern.RawText.EndsWith("openapi.json", StringComparison.Ordinal))
            .ToList();
        Assert.Equal(6, routes.Count);
        foreach (var route in routes)
        {
            var method = route.Metadata.GetMetadata<HttpMethodMetadata>()!.HttpMethods.Single().ToLowerInvariant();
            Assert.True(paths.TryGetProperty(route.RoutePattern.RawText!, out var path), route.RoutePattern.RawText);
            Assert.True(path.TryGetProperty(method, out var op), $"{method} {route.RoutePattern.RawText}");
            Assert.True(op.GetProperty("responses").TryGetProperty("401", out _));
        }

        // Every operation that takes a form documents it with an example; every edit op has a schema.
        foreach (var name in new[] { "convert", "merge", "edit" })
        {
            var content = paths.GetProperty($"/api/v1/documents/{name}").GetProperty("post").GetProperty("requestBody").GetProperty("content").GetProperty("multipart/form-data");
            Assert.True(content.TryGetProperty("example", out _), name);
        }

        var schemas = doc.GetProperty("components").GetProperty("schemas");
        foreach (var schema in new[] { "SetControl", "SetBookmark", "ReplaceText", "SetProperties", "SetCell" })
        {
            var op = schemas.GetProperty(schema).GetProperty("properties").GetProperty("op").GetProperty("enum")[0].GetString()!;
            Assert.Equal(schema, char.ToUpperInvariant(op[0]) + op[1..]);
        }
    }

    [Theory]
    [InlineData("RestApi:Keys:0:Sha256", "erp-key-in-clear-text")]
    [InlineData("RestApi:Keys:1:Name", "erp")]
    [InlineData("RestApi:Keys:1:Scopes:0", "admin")]
    [InlineData("RestApi:MaxUploadBytes", "0")]
    public void InvalidConfigurationStopsTheServer(string key, string value)
    {
        using var app = new TestApp(settings: RestApi.Settings((key, value)));
        var e = Assert.ThrowsAny<Exception>(() => app.CreateClient());
        Assert.Contains("RestApi", e.ToString(), StringComparison.Ordinal);
    }
}

/// <summary>
/// The REST API through the queue, the worker and the real <c>bo-cli</c> on corpus files. Skipped (returns early) when
/// bo-cli is not built; CI builds it before <c>dotnet test</c>.
/// </summary>
public sealed class RestApiEndToEndTests : IDisposable
{
    private const string Docx = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";
    private readonly TestApp? _app;

    public RestApiEndToEndTests()
    {
        if (BoCliConverterTests.FindCli() is { } cli)
        {
            _app = new TestApp(settings: RestApi.Settings(
                ("BoCli:ExecutablePath", cli), ("BoCli:Timeout", "00:02:00"), ("Worker:WaitTimeout", "00:02:00"), ("RestApi:MaxUploadBytes", "52428800")));
        }
    }

    public void Dispose() => _app?.Dispose();

    private static byte[] Corpus(string relative) => File.ReadAllBytes(Path.Combine(BoCliConverterTests.RepoRoot(), "tests", "corpus", relative));

    private async Task<byte[]> OkAsync(string operation, MultipartFormDataContent form, string contentType)
    {
        var res = await _app!.WithKey(RestApi.FullKey).PostDocAsync(operation, form);
        var body = await res.Content.ReadAsByteArrayAsync();
        Assert.True(res.StatusCode == HttpStatusCode.OK, $"{(int)res.StatusCode}: {Encoding.UTF8.GetString(body)}");
        Assert.Equal(contentType, res.Content.Headers.ContentType?.MediaType);
        return body;
    }

    private static string Entry(byte[] zip, string name)
    {
        using var archive = new ZipArchive(new MemoryStream(zip));
        using var reader = new StreamReader(archive.GetEntry(name)!.Open(), Encoding.UTF8);
        return reader.ReadToEnd();
    }

    [Fact]
    public async Task ConvertsToPdfA()
    {
        if (_app is null)
        {
            return;
        }

        var pdf = await OkAsync("convert", RestApi.Form([("file", "Công văn.docx", Corpus("doc/phase1-sample.docx"))], ("pdfa", "2b"), ("tagged", "true"), ("bookmarks", "true")), "application/pdf");
        Assert.Equal("%PDF-"u8.ToArray(), pdf.Take(5).ToArray());
        Assert.Contains("pdfaid", Encoding.Latin1.GetString(pdf), StringComparison.Ordinal);
    }

    [Fact]
    public async Task MergesOneFilePerRecordAndOneDocument()
    {
        if (_app is null)
        {
            return;
        }

        var template = Corpus("doc/merge-01-letter.docx");
        var zip = await OkAsync(
            "merge",
            RestApi.Form([("template", "Thư.docx", template), ("data", "data.csv", Corpus("doc/merge-data.csv"))], ("output", "zip"), ("to", "docx"), ("pattern", "Thư «Họ tên»")),
            "application/zip");
        using (var archive = new ZipArchive(new MemoryStream(zip)))
        {
            Assert.Equal(["Thư Nguyễn Văn An.docx", "Thư Trần Thị Bình.docx", "Thư Lê Văn Cường.docx"], archive.Entries.Select(e => e.FullName));
            using var ms = new MemoryStream();
            using (var s = archive.Entries[1].Open())
            {
                await s.CopyToAsync(ms);
            }

            var letter = Entry(ms.ToArray(), "word/document.xml");
            Assert.Contains("Trần Thị Bình", letter, StringComparison.Ordinal);
            Assert.DoesNotContain("Nguyễn Văn An", letter, StringComparison.Ordinal);
        }

        // Records as JSON, one merged document.
        const string Records = """[{"Họ tên":"Phạm Thị Dung","ChucVu":"Kế toán trưởng","DonVi":"Công ty Sao Xanh","SoTien":"900.000"}]""";
        var merged = await OkAsync("merge", RestApi.Form([("template", "Thư.docx", template)], ("records", Records)), Docx);
        Assert.Contains("Phạm Thị Dung", Entry(merged, "word/document.xml"), StringComparison.Ordinal);
        var pdf = await OkAsync("merge", RestApi.Form([("template", "Thư.docx", template)], ("records", Records), ("to", "pdf")), "application/pdf");
        Assert.Equal("%PDF-"u8.ToArray(), pdf.Take(5).ToArray());
    }

    [Fact]
    public async Task FillsDocumentAndWorkbookTemplates()
    {
        if (_app is null)
        {
            return;
        }

        const string Edits = """
            [{"op":"setControl","tag":"ho_ten","value":"Nguyễn Văn An"},
             {"op":"setControl","title":"Phòng ban","value":"Kế toán"},
             {"op":"setProperties","props":{"title":"Hợp đồng lao động"}}]
            """;
        var template = Corpus("doc/controls-01-forms.docx");
        var res = await _app.WithKey(RestApi.FullKey).PostDocAsync("edit", RestApi.Form([("file", "Hợp đồng.docx", template)], ("edits", Edits)));
        Assert.Equal(HttpStatusCode.OK, res.StatusCode);
        Assert.Equal("3", res.Headers.GetValues("X-Edits-Applied").Single());
        Assert.Equal("0", res.Headers.GetValues("X-Edits-Unmatched").Single());
        var filled = await res.Content.ReadAsByteArrayAsync();
        var xml = Entry(filled, "word/document.xml");
        Assert.Contains("Nguyễn Văn An", xml, StringComparison.Ordinal);
        Assert.DoesNotContain("Nhập họ tên", xml, StringComparison.Ordinal);
        Assert.Contains("Hợp đồng lao động", Entry(filled, "docProps/core.xml"), StringComparison.Ordinal);

        // Strict (default): an edit that matches nothing fails; strict=false reports it.
        const string Missing = """[{"op":"setBookmark","name":"KhongCo","text":"x"}]""";
        var strict = await _app.WithKey(RestApi.FullKey).PostDocAsync("edit", RestApi.Form([("file", "a.docx", template)], ("edits", Missing)));
        Assert.Equal(HttpStatusCode.UnprocessableEntity, strict.StatusCode);
        Assert.Contains("matched nothing", await strict.DetailAsync(), StringComparison.Ordinal);
        var lenient = await _app.WithKey(RestApi.FullKey).PostDocAsync("edit", RestApi.Form([("file", "a.docx", template)], ("edits", Missing), ("strict", "false"), ("to", "pdf")));
        Assert.Equal(HttpStatusCode.OK, lenient.StatusCode);
        Assert.Equal("1", lenient.Headers.GetValues("X-Edits-Unmatched").Single());
        Assert.Equal("application/pdf", lenient.Content.Headers.ContentType?.MediaType);

        // A bookmark of a document, a named cell of a workbook.
        var marked = await OkAsync("edit", RestApi.Form([("file", "b.docx", Corpus("doc/links-02-bookmarks.docx"))], ("edits", """[{"op":"setBookmark","name":"_Toc1","text":"Mục đích của đề án"}]""")), Docx);
        Assert.Contains("Mục đích của đề án", Entry(marked, "word/document.xml"), StringComparison.Ordinal);
        var book = await OkAsync(
            "edit",
            RestApi.Form([("file", "Tỷ giá.xlsx", Corpus("sheet/names-1.xlsx"))], ("edits", """[{"op":"setCell","ref":"TyGia","value":25000}]""")),
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet");
        Assert.Contains("<v>25000</v>", Entry(book, "xl/worksheets/sheet1.xml"), StringComparison.Ordinal);
    }

    [Fact]
    public async Task InspectsRealFiles()
    {
        if (_app is null)
        {
            return;
        }

        var json = JsonDocument.Parse(await OkAsync("inspect", RestApi.Form([("file", "tệp.bin", Corpus("sheet/names-1.xlsx"))]), "application/json")).RootElement;
        Assert.Equal("xlsx", json.GetProperty("format").GetString());
        Assert.Equal("spreadsheet", json.GetProperty("kind").GetString());
        Assert.False(json.GetProperty("encrypted").GetBoolean());
        Assert.False(json.GetProperty("macros").GetBoolean());
        Assert.Equal(2, json.GetProperty("sheets").GetArrayLength());

        var junk = await _app.WithKey(RestApi.FullKey).PostDocAsync("inspect", RestApi.Form([("file", "junk.docx", "PK\u0003\u0004 not a zip"u8.ToArray())]));
        Assert.Equal(HttpStatusCode.UnprocessableEntity, junk.StatusCode);
    }
}
