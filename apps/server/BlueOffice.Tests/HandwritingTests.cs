using System.Collections.Concurrent;
using System.Net;
using System.Net.Http.Json;
using System.Text;
using System.Text.Json;
using BlueOffice.Api.Handwriting;
using Microsoft.AspNetCore.Mvc.Testing;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.DependencyInjection.Extensions;
using Microsoft.Extensions.Logging;
using Xunit;

namespace BlueOffice.Tests;

/// <summary>The handwriting recognition proxy <c>/api/handwriting</c> (CORE-INK-002).</summary>
public sealed class HandwritingTests
{
    private static readonly Uri Capabilities = new("/api/handwriting/capabilities", UriKind.Relative);

    private static Uri Recognize(string mode = "text", string lang = "vi-VN", string? session = null) =>
        new($"/api/handwriting?mode={mode}&lang={lang}{(session is null ? "" : "&session=" + session)}", UriKind.Relative);

    private static readonly object Ink = new { strokes = new[] { new { points = new[] { new[] { 10.0, 20.0, 0 }, new[] { 30.0, 25.0, 16 } } } } };

    private static StringContent Json(object body) => new(JsonSerializer.Serialize(body), Encoding.UTF8, "application/json");

    private sealed class FakeApi(Func<HttpRequestMessage, string, HttpResponseMessage> answer) : HttpMessageHandler
    {
        public ConcurrentQueue<(string Url, string Body, string? Auth)> Requests { get; } = new();

        protected override async Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken)
        {
            var body = await request.Content!.ReadAsStringAsync(cancellationToken);
            Requests.Enqueue((request.RequestUri!.ToString(), body, request.Headers.TryGetValues("Authorization", out var a) ? a.First() : null));
            return answer(request, body);
        }
    }

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

    private sealed class EchoEngine : IHandwritingEngine
    {
        public List<HandwritingInput> Calls { get; } = [];

        public Task<HandwritingReading> RecognizeAsync(HandwritingInput input, CancellationToken ct)
        {
            Calls.Add(input);
            return Task.FromResult(new HandwritingReading(input.Mode == "math" ? "x^2" : "xin chào", ["xin chao"]));
        }
    }

    private static HttpResponseMessage Answer(object body, HttpStatusCode status = HttpStatusCode.OK) =>
        new(status) { Content = new StringContent(JsonSerializer.Serialize(body), Encoding.UTF8, "application/json") };

    private static WebApplicationFactory<Program> With(TestApp app, FakeApi api, CaptureLogs? logs = null) =>
        app.WithWebHostBuilder(b => b.ConfigureServices(s =>
        {
            s.AddHttpClient(HandwritingEndpoints.HttpClientName).ConfigurePrimaryHttpMessageHandler(() => api);
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
        var api = new FakeApi((_, _) => Answer(new { text = "x" }));
        using var f = With(app, api);
        var client = Signed(f);
        var r = await client.PostAsync(Recognize(), Json(Ink));
        Assert.Equal(HttpStatusCode.NotFound, r.StatusCode);
        Assert.Contains("unavailable", await r.Content.ReadAsStringAsync(), StringComparison.Ordinal);
        Assert.Equal(HttpStatusCode.NotFound, (await client.GetAsync(Capabilities)).StatusCode);
        Assert.Empty(api.Requests);
    }

    [Fact]
    public async Task ForwardsTheStrokes()
    {
        using var app = new TestApp(settings: new()
        {
            ["Handwriting:Provider"] = "Http",
            ["Handwriting:Modes:0"] = "text",
            ["Handwriting:Modes:1"] = "math",
            ["Handwriting:Http:Url"] = "http://hwr.internal/{{mode}}?lang={{lang}}",
            ["Handwriting:Http:Headers:Authorization"] = "Key secret-9",
            ["Handwriting:Http:ResponsePath"] = "results[0].label",
            ["Handwriting:Http:AlternativesPath"] = "results[*].label",
            ["Handwriting:Http:LanguageMap:vi"] = "vi-VN",
        });
        var api = new FakeApi((_, _) => Answer(new { results = new[] { new { label = " xin chào " }, new { label = "xin chao" } } }));
        using var f = With(app, api);
        var client = Signed(f);
        var caps = await client.GetFromJsonAsync<JsonElement>(Capabilities);
        Assert.Equal("[\"text\",\"math\"]", caps.GetProperty("modes").GetRawText());
        Assert.Equal("[\"vi\",\"en\"]", caps.GetProperty("languages").GetRawText());
        Assert.Equal("latex", caps.GetProperty("mathFormat").GetString());

        var r = await client.PostAsync(Recognize(), Json(Ink));
        Assert.Equal(HttpStatusCode.OK, r.StatusCode);
        var got = await r.Content.ReadFromJsonAsync<JsonElement>();
        Assert.Equal("xin chào", got.GetProperty("text").GetString());
        Assert.Equal("[\"xin chao\"]", got.GetProperty("alternatives").GetRawText());
        Assert.True(api.Requests.TryDequeue(out var sent));
        Assert.Equal("http://hwr.internal/text?lang=vi-VN", sent.Url);
        Assert.Equal("Key secret-9", sent.Auth);
        var body = JsonDocument.Parse(sent.Body).RootElement;
        Assert.Equal("text", body.GetProperty("mode").GetString());
        Assert.Equal("vi-VN", body.GetProperty("language").GetString());
        Assert.Equal("[[10,20,0],[30,25,16]]", body.GetProperty("strokes")[0].GetProperty("points").GetRawText());

        // Math answers carry the configured notation.
        r = await client.PostAsync(Recognize("math", "en-US"), Json(Ink));
        Assert.Equal("latex", (await r.Content.ReadFromJsonAsync<JsonElement>()).GetProperty("format").GetString());
        Assert.True(api.Requests.TryDequeue(out sent));
        Assert.Equal("http://hwr.internal/math?lang=en", sent.Url);
    }

    [Fact]
    public async Task UsesARegisteredEngineAndOnlyItsModes()
    {
        using var app = new TestApp(settings: new()
        {
            ["Handwriting:Provider"] = "Custom",
            ["Handwriting:Modes:0"] = "math",
            ["Handwriting:MathFormat"] = "unicodeMath",
        });
        var engine = new EchoEngine();
        using var f = app.WithWebHostBuilder(b => b.ConfigureServices(s => s.AddSingleton<IHandwritingEngine>(engine)));
        var client = Signed(f);
        var r = await client.PostAsync(Recognize("math"), Json(Ink));
        var got = await r.Content.ReadFromJsonAsync<JsonElement>();
        Assert.Equal("x^2", got.GetProperty("text").GetString());
        Assert.Equal("unicodeMath", got.GetProperty("format").GetString());
        Assert.Equal("vi", engine.Calls.Single().Lang);
        Assert.Equal(2, engine.Calls.Single().Strokes[0].Points.Count);
        // Text is not offered here.
        r = await client.PostAsync(Recognize("text"), Json(Ink));
        Assert.Equal(HttpStatusCode.BadRequest, r.StatusCode);
        Assert.Contains("unsupportedMode", await r.Content.ReadAsStringAsync(), StringComparison.Ordinal);
    }

    [Fact]
    public async Task RefusesStrangersAndBadInput()
    {
        using var app = new TestApp(settings: new()
        {
            ["Handwriting:Provider"] = "Http",
            ["Handwriting:Http:Url"] = "http://hwr.internal/recognize",
            ["Handwriting:MaxPoints"] = "3",
        });
        var api = new FakeApi((_, _) => Answer(new { text = "x" }));
        using var f = With(app, api);
        var anon = f.CreateClient();
        Assert.Equal(HttpStatusCode.Unauthorized, (await anon.PostAsync(Recognize(), Json(Ink))).StatusCode);
        Assert.Equal(HttpStatusCode.Unauthorized, (await anon.PostAsync(Recognize(session: "nope"), Json(Ink))).StatusCode);
        Assert.Equal(HttpStatusCode.Unauthorized, (await anon.GetAsync(Capabilities)).StatusCode);
        var client = Signed(f);
        Assert.Equal(HttpStatusCode.BadRequest, (await client.PostAsync(Recognize(lang: "fr"), Json(Ink))).StatusCode);
        Assert.Equal(HttpStatusCode.BadRequest, (await client.PostAsync(Recognize("math"), Json(Ink))).StatusCode);
        Assert.Equal(HttpStatusCode.UnsupportedMediaType, (await client.PostAsync(Recognize(), new StringContent("x", Encoding.UTF8, "text/plain"))).StatusCode);
        Assert.Equal(HttpStatusCode.BadRequest, (await client.PostAsync(Recognize(), new StringContent("{", Encoding.UTF8, "application/json"))).StatusCode);
        Assert.Equal(HttpStatusCode.BadRequest, (await client.PostAsync(Recognize(), Json(new { strokes = Array.Empty<object>() }))).StatusCode);
        Assert.Equal(HttpStatusCode.BadRequest, (await client.PostAsync(Recognize(), Json(new { strokes = new[] { new { points = new[] { new[] { 1.0 } } } } }))).StatusCode);
        var many = new { strokes = new[] { new { points = new[] { new[] { 1.0, 2 }, new[] { 3.0, 4 }, new[] { 5.0, 6 }, new[] { 7.0, 8 } } } } };
        Assert.Equal(HttpStatusCode.RequestEntityTooLarge, (await client.PostAsync(Recognize(), Json(many))).StatusCode);
        Assert.Empty(api.Requests);
    }

    [Fact]
    public async Task UpstreamFailuresAreReportedWithoutLoggingInkOrText()
    {
        const string Secret = "Mật khẩu kho 7731";
        using var app = new TestApp(settings: new()
        {
            ["Handwriting:Provider"] = "Http",
            ["Handwriting:Http:Url"] = "http://hwr.internal/recognize",
        });
        var status = HttpStatusCode.InternalServerError;
        var api = new FakeApi((_, _) => status == HttpStatusCode.OK ? Answer(new { text = Secret }) : Answer(new { error = "boom" }, status));
        var logs = new CaptureLogs();
        using var f = With(app, api, logs);
        var client = Signed(f);
        var r = await client.PostAsync(Recognize(), Json(Ink));
        Assert.Equal(HttpStatusCode.BadGateway, r.StatusCode);
        Assert.Contains("upstream", await r.Content.ReadAsStringAsync(), StringComparison.Ordinal);
        status = HttpStatusCode.TooManyRequests;
        Assert.Equal(HttpStatusCode.TooManyRequests, (await client.PostAsync(Recognize(), Json(Ink))).StatusCode);
        status = HttpStatusCode.OK;
        r = await client.PostAsync(Recognize(), Json(Ink));
        Assert.Equal(Secret, (await r.Content.ReadFromJsonAsync<JsonElement>()).GetProperty("text").GetString());
        Assert.Contains(logs.Lines, l => l.Contains("HTTP 500", StringComparison.Ordinal));
        Assert.DoesNotContain(logs.Lines, l => l.Contains(Secret, StringComparison.Ordinal) || l.Contains("[10,20", StringComparison.Ordinal));
    }

    [Fact]
    public void HttpProviderNeedsAUrl()
    {
        using var app = new TestApp(settings: new() { ["Handwriting:Provider"] = "Http" });
        Assert.ThrowsAny<Exception>(() => app.CreateClient());
    }
}
