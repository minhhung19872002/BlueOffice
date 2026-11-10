using System.Collections.Concurrent;
using System.Net;
using System.Net.Http.Json;
using System.Text;
using System.Text.Json;
using BlueOffice.Api.Translation;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Mvc.Testing;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.DependencyInjection.Extensions;
using Microsoft.Extensions.Logging;
using Xunit;

namespace BlueOffice.Tests;

/// <summary>The translation proxy <c>/api/translate</c> (CORE-LANG-001).</summary>
public sealed class TranslationTests
{
    private static readonly Uri Translate = new("/api/translate", UriKind.Relative);
    private static readonly Uri Languages = new("/api/translate/languages", UriKind.Relative);

    /// <summary>A fake translation API recording the requests it gets.</summary>
    private sealed class FakeApi(Func<JsonElement, HttpResponseMessage> answer) : HttpMessageHandler
    {
        public ConcurrentQueue<(string Url, string Body, string? Auth)> Requests { get; } = new();

        protected override async Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken)
        {
            var body = await request.Content!.ReadAsStringAsync(cancellationToken);
            Requests.Enqueue((request.RequestUri!.ToString(), body, request.Headers.TryGetValues("Authorization", out var a) ? a.First() : null));
            return answer(JsonDocument.Parse(body).RootElement);
        }
    }

    /// <summary>Every log line the app writes.</summary>
    private sealed class CaptureLogs : ILoggerProvider, ILoggerFactory
    {
        public ConcurrentQueue<string> Lines { get; } = new();

        public ILogger CreateLogger(string categoryName) => new Logger(this, categoryName);

        public void AddProvider(ILoggerProvider provider)
        {
        }

        public void Dispose()
        {
        }

        private sealed class Logger(CaptureLogs owner, string category) : ILogger
        {
            public IDisposable? BeginScope<TState>(TState state)
                where TState : notnull => null;

            public bool IsEnabled(LogLevel logLevel) => true;

            public void Log<TState>(LogLevel logLevel, EventId eventId, TState state, Exception? exception, Func<TState, Exception?, string> formatter) =>
                owner.Lines.Enqueue($"{category}: {formatter(state, exception)} {exception}");
        }
    }

    private static HttpResponseMessage Json(object body, HttpStatusCode status = HttpStatusCode.OK) =>
        new(status) { Content = new StringContent(JsonSerializer.Serialize(body), Encoding.UTF8, "application/json") };

    private static WebApplicationFactory<Program> With(TestApp app, FakeApi api, CaptureLogs? logs = null) =>
        app.WithWebHostBuilder(b => b.ConfigureServices(s =>
        {
            s.AddHttpClient(TranslationEndpoints.HttpClientName).ConfigurePrimaryHttpMessageHandler(() => api);
            if (logs is not null)
            {
                s.RemoveAll<ILoggerFactory>();
                s.AddSingleton<ILoggerFactory>(logs);
            }
        }));

    private static HttpClient Signed(WebApplicationFactory<Program> f)
    {
        var c = f.CreateClient();
        c.DefaultRequestHeaders.Authorization = new("Bearer", TestApp.UserToken("alice", "Alice"));
        return c;
    }

    [Fact]
    public async Task OffByDefault()
    {
        using var app = new TestApp();
        var client = app.ClientFor("alice", "Alice");
        var r = await client.PostAsJsonAsync(Translate, new { texts = new[] { "Xin chào" }, from = "vi", to = "en" });
        Assert.Equal(HttpStatusCode.NotFound, r.StatusCode);
        Assert.Contains("unavailable", await r.Content.ReadAsStringAsync(), StringComparison.Ordinal);
        Assert.Equal(HttpStatusCode.NotFound, (await client.GetAsync(Languages)).StatusCode);
    }

    [Fact]
    public async Task ForwardsWithTheConfiguredMapping()
    {
        using var app = new TestApp(settings: new()
        {
            ["Translation:Provider"] = "Http",
            ["Translation:Http:Url"] = "http://translator.internal/v2/translate",
            ["Translation:Http:Headers:Authorization"] = "DeepKey secret-123",
            ["Translation:Http:BodyTemplate"] = "{\"text\":{{texts}},\"source_lang\":{{from}},\"target_lang\":{{to}}}",
            ["Translation:Http:ResponsePath"] = "data.translations[*].translatedText",
            ["Translation:Http:LanguageMap:en"] = "EN-US",
            ["Translation:Http:AutoSource"] = "",
        });
        var api = new FakeApi(req => Json(new
        {
            data = new { translations = req.GetProperty("text").EnumerateArray().Select(t => new { translatedText = "EN:" + t.GetString() }).ToArray() },
        }));
        using var f = With(app, api);
        var client = Signed(f);
        var langs = await client.GetFromJsonAsync<JsonElement>(Languages);
        Assert.Equal("[\"vi\",\"en\"]", langs.GetProperty("languages").GetRawText());
        var r = await client.PostAsJsonAsync(Translate, new { texts = new[] { "Xin chào", "Tạm biệt \"bạn\"" }, from = "vi", to = "en" });
        Assert.Equal(HttpStatusCode.OK, r.StatusCode);
        var got = await r.Content.ReadFromJsonAsync<JsonElement>();
        Assert.Equal(["EN:Xin chào", "EN:Tạm biệt \"bạn\""], got.GetProperty("texts").EnumerateArray().Select(e => e.GetString()));
        Assert.True(api.Requests.TryDequeue(out var sent));
        Assert.Equal("http://translator.internal/v2/translate", sent.Url);
        Assert.Equal("DeepKey secret-123", sent.Auth);
        var body = JsonDocument.Parse(sent.Body).RootElement;
        Assert.Equal("vi", body.GetProperty("source_lang").GetString());
        Assert.Equal("EN-US", body.GetProperty("target_lang").GetString());
        Assert.Equal(2, body.GetProperty("text").GetArrayLength());

        // `auto` sends the configured AutoSource.
        await client.PostAsJsonAsync(Translate, new { texts = new[] { "Xin chào" }, from = "auto", to = "en" });
        Assert.True(api.Requests.TryDequeue(out sent));
        Assert.Equal("", JsonDocument.Parse(sent.Body).RootElement.GetProperty("source_lang").GetString());
    }

    [Fact]
    public async Task PerTextApis()
    {
        using var app = new TestApp(settings: new()
        {
            ["Translation:Provider"] = "Http",
            ["Translation:Http:Url"] = "http://translator.internal/translate",
            ["Translation:Http:PerText"] = "true",
            ["Translation:Http:BodyTemplate"] = "{\"q\":{{text}},\"source\":{{from}},\"target\":{{to}}}",
            ["Translation:Http:ResponsePath"] = "translatedText",
        });
        var api = new FakeApi(req => Json(new { translatedText = req.GetProperty("q").GetString()!.ToUpperInvariant() }));
        using var f = With(app, api);
        var r = await Signed(f).PostAsJsonAsync(Translate, new { texts = new[] { "một", "hai" }, from = "vi", to = "en" });
        var got = await r.Content.ReadFromJsonAsync<JsonElement>();
        Assert.Equal(["MỘT", "HAI"], got.GetProperty("texts").EnumerateArray().Select(e => e.GetString()));
        Assert.Equal(2, api.Requests.Count);
    }

    [Fact]
    public async Task RefusesStrangersAndBadInput()
    {
        using var app = new TestApp(settings: new()
        {
            ["Translation:Provider"] = "Http",
            ["Translation:Http:Url"] = "http://translator.internal/translate",
            ["Translation:MaxTexts"] = "2",
            ["Translation:MaxChars"] = "10",
        });
        var api = new FakeApi(_ => Json(new { translatedText = new[] { "x" } }));
        using var f = With(app, api);
        // Neither signed in nor in an editor session.
        var anon = f.CreateClient();
        Assert.Equal(HttpStatusCode.Unauthorized, (await anon.PostAsJsonAsync(Translate, new { texts = new[] { "a" }, from = "vi", to = "en" })).StatusCode);
        Assert.Equal(HttpStatusCode.Unauthorized, (await anon.PostAsJsonAsync(Translate, new { texts = new[] { "a" }, from = "vi", to = "en", session = "nope" })).StatusCode);
        Assert.Equal(HttpStatusCode.Unauthorized, (await anon.GetAsync(Languages)).StatusCode);
        var client = Signed(f);
        async Task<(HttpStatusCode, string)> Post(object body)
        {
            var r = await client.PostAsJsonAsync(Translate, body);
            return (r.StatusCode, await r.Content.ReadAsStringAsync());
        }

        var (s, b) = await Post(new { texts = new[] { "a" }, from = "vi", to = "fr" });
        Assert.Equal(HttpStatusCode.BadRequest, s);
        Assert.Contains("unsupportedLanguage", b, StringComparison.Ordinal);
        (s, b) = await Post(new { texts = new[] { "a" }, from = "en", to = "en" });
        Assert.Contains("sameLanguage", b, StringComparison.Ordinal);
        (s, _) = await Post(new { texts = Array.Empty<string>(), from = "vi", to = "en" });
        Assert.Equal(HttpStatusCode.BadRequest, s);
        (s, b) = await Post(new { texts = new[] { "a", "b", "c" }, from = "vi", to = "en" });
        Assert.Equal(HttpStatusCode.RequestEntityTooLarge, s);
        Assert.Contains("tooLarge", b, StringComparison.Ordinal);
        (s, _) = await Post(new { texts = new[] { "một hai ba bốn" }, from = "vi", to = "en" });
        Assert.Equal(HttpStatusCode.RequestEntityTooLarge, s);
        Assert.Empty(api.Requests);
    }

    [Fact]
    public async Task UpstreamFailuresAreReportedWithoutLoggingTheText()
    {
        const string Secret = "Hợp đồng mật số 42";
        using var app = new TestApp(settings: new()
        {
            ["Translation:Provider"] = "Http",
            ["Translation:Http:Url"] = "http://translator.internal/translate",
        });
        var status = HttpStatusCode.InternalServerError;
        var api = new FakeApi(_ => Json(new { error = "boom" }, status));
        var logs = new CaptureLogs();
        using var f = With(app, api, logs);
        var client = Signed(f);
        var r = await client.PostAsJsonAsync(Translate, new { texts = new[] { Secret }, from = "vi", to = "en" });
        Assert.Equal(HttpStatusCode.BadGateway, r.StatusCode);
        Assert.Contains("upstream", await r.Content.ReadAsStringAsync(), StringComparison.Ordinal);
        status = HttpStatusCode.TooManyRequests;
        r = await client.PostAsJsonAsync(Translate, new { texts = new[] { Secret }, from = "vi", to = "en" });
        Assert.Equal(HttpStatusCode.TooManyRequests, r.StatusCode);
        // A wrong number of translations is an upstream error too.
        status = HttpStatusCode.OK;
        r = await client.PostAsJsonAsync(Translate, new { texts = new[] { Secret }, from = "vi", to = "en" });
        Assert.Equal(HttpStatusCode.BadGateway, r.StatusCode);
        // The failure is logged (the capture works) but never the text.
        Assert.Contains(logs.Lines, l => l.Contains("HTTP 500", StringComparison.Ordinal));
        Assert.DoesNotContain(logs.Lines, l => l.Contains(Secret, StringComparison.Ordinal) || l.Contains("Hợp đồng", StringComparison.Ordinal));
    }

    [Theory]
    [InlineData("translatedText", "{\"translatedText\":[\"a\",\"b\"]}", "a,b")]
    [InlineData("translations[*].text", "{\"translations\":[{\"text\":\"a\"},{\"text\":\"b\"}]}", "a,b")]
    [InlineData("data.translations[1].t", "{\"data\":{\"translations\":[{\"t\":\"a\"},{\"t\":\"b\"}]}}", "b")]
    [InlineData("", "[\"a\"]", "a")]
    [InlineData("missing", "{\"x\":1}", "")]
    public void ResponsePaths(string path, string json, string expected)
    {
        var found = HttpTranslationEngine.Select(JsonDocument.Parse(json).RootElement, path);
        var texts = found.SelectMany(e => e.ValueKind == JsonValueKind.Array ? e.EnumerateArray().Select(x => x.GetString()) : [e.GetString()]);
        Assert.Equal(expected, string.Join(',', texts));
    }

    [Fact]
    public void BodyTemplateEncodesValues()
    {
        var o = new HttpTranslationOptions { LanguageMap = { ["en"] = "EN-GB" } };
        var body = HttpTranslationEngine.Body(o, JsonSerializer.Serialize(new[] { "a\"b" }), null, "auto", "en");
        var e = JsonDocument.Parse(body).RootElement;
        Assert.Equal("a\"b", e.GetProperty("q")[0].GetString());
        Assert.Equal("auto", e.GetProperty("source").GetString());
        Assert.Equal("EN-GB", e.GetProperty("target").GetString());
    }

    [Fact]
    public void HttpProviderNeedsAUrl()
    {
        using var app = new TestApp(settings: new() { ["Translation:Provider"] = "Http" });
        Assert.ThrowsAny<Exception>(() => app.CreateClient());
    }
}
