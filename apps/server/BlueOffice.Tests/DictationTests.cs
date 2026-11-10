using System.Collections.Concurrent;
using System.Net;
using System.Net.Http.Headers;
using System.Net.Http.Json;
using System.Text;
using System.Text.Json;
using BlueOffice.Api.Dictation;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Mvc.Testing;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.DependencyInjection.Extensions;
using Microsoft.Extensions.Logging;
using Xunit;

namespace BlueOffice.Tests;

/// <summary>The dictation proxy <c>/api/dictation</c> (CORE-LANG-002).</summary>
public sealed class DictationTests
{
    private static readonly Uri Languages = new("/api/dictation/languages", UriKind.Relative);

    private static Uri Dictate(string lang = "vi-VN", string? session = null) =>
        new($"/api/dictation?lang={lang}{(session is null ? "" : "&session=" + session)}", UriKind.Relative);

    /// <summary>A small WAV-looking body (the proxy does not decode it).</summary>
    private static readonly byte[] Audio = [.. Encoding.ASCII.GetBytes("RIFF\0\0\0\0WAVE"), .. Enumerable.Range(0, 64).Select(i => (byte)i)];

    private static ByteArrayContent Wav(byte[]? bytes = null, string type = "audio/wav")
    {
        var c = new ByteArrayContent(bytes ?? Audio);
        c.Headers.ContentType = new MediaTypeHeaderValue(type);
        return c;
    }

    /// <summary>A fake speech-to-text service recording what it gets.</summary>
    private sealed class FakeApi(Func<HttpRequestMessage, byte[], HttpResponseMessage> answer) : HttpMessageHandler
    {
        public ConcurrentQueue<(string Url, string? ContentType, byte[] Body, string? Auth)> Requests { get; } = new();

        protected override async Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken)
        {
            var body = await request.Content!.ReadAsByteArrayAsync(cancellationToken);
            Requests.Enqueue((
                request.RequestUri!.ToString(),
                request.Content.Headers.ContentType?.ToString(),
                body,
                request.Headers.TryGetValues("Authorization", out var a) ? a.First() : null));
            return answer(request, body);
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

    /// <summary>A deployment's own engine (a local model).</summary>
    private sealed class EchoEngine : IDictationEngine
    {
        public List<(int Bytes, string Type, string Lang)> Calls { get; } = [];

        public Task<DictationTranscript> TranscribeAsync(ReadOnlyMemory<byte> audio, string contentType, string lang, CancellationToken ct)
        {
            Calls.Add((audio.Length, contentType, lang));
            return Task.FromResult(new DictationTranscript("xin chào chấm", null));
        }
    }

    private static HttpResponseMessage Json(object body, HttpStatusCode status = HttpStatusCode.OK) =>
        new(status) { Content = new StringContent(JsonSerializer.Serialize(body), Encoding.UTF8, "application/json") };

    private static WebApplicationFactory<Program> With(TestApp app, FakeApi api, CaptureLogs? logs = null) =>
        app.WithWebHostBuilder(b => b.ConfigureServices(s =>
        {
            s.AddHttpClient(DictationEndpoints.HttpClientName).ConfigurePrimaryHttpMessageHandler(() => api);
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
        var api = new FakeApi((_, _) => Json(new { text = "x" }));
        using var f = With(app, api);
        var client = Signed(f);
        var r = await client.PostAsync(Dictate(), Wav());
        Assert.Equal(HttpStatusCode.NotFound, r.StatusCode);
        Assert.Contains("unavailable", await r.Content.ReadAsStringAsync(), StringComparison.Ordinal);
        Assert.Equal(HttpStatusCode.NotFound, (await client.GetAsync(Languages)).StatusCode);
        // Nothing was sent anywhere.
        Assert.Empty(api.Requests);
    }

    [Fact]
    public async Task ForwardsTheAudioRaw()
    {
        using var app = new TestApp(settings: new()
        {
            ["Dictation:Provider"] = "Http",
            ["Dictation:Http:Url"] = "http://stt.internal/recognize?language={{lang}}",
            ["Dictation:Http:Headers:Authorization"] = "Key secret-123",
            ["Dictation:Http:ResponsePath"] = "results[0].alternatives[0].transcript",
            ["Dictation:Http:ConfidencePath"] = "results[0].alternatives[0].confidence",
            ["Dictation:Http:LanguageMap:vi"] = "vi-VN",
        });
        var api = new FakeApi((_, _) => Json(new { results = new[] { new { alternatives = new[] { new { transcript = " xin chào chấm ", confidence = 0.87 } } } } }));
        using var f = With(app, api);
        var client = Signed(f);
        var langs = await client.GetFromJsonAsync<JsonElement>(Languages);
        Assert.Equal("[\"vi\",\"en\"]", langs.GetProperty("languages").GetRawText());
        var r = await client.PostAsync(Dictate(), Wav());
        Assert.Equal(HttpStatusCode.OK, r.StatusCode);
        var got = await r.Content.ReadFromJsonAsync<JsonElement>();
        Assert.Equal("xin chào chấm", got.GetProperty("text").GetString());
        Assert.Equal(0.87, got.GetProperty("confidence").GetDouble(), 3);
        Assert.True(api.Requests.TryDequeue(out var sent));
        Assert.Equal("http://stt.internal/recognize?language=vi-VN", sent.Url);
        Assert.Equal("audio/wav", sent.ContentType);
        Assert.Equal(Audio, sent.Body);
        Assert.Equal("Key secret-123", sent.Auth);

        // English: unmapped, the primary subtag.
        await client.PostAsync(Dictate("en-US"), Wav());
        Assert.True(api.Requests.TryDequeue(out sent));
        Assert.Equal("http://stt.internal/recognize?language=en", sent.Url);
    }

    [Fact]
    public async Task ForwardsAMultipartForm()
    {
        using var app = new TestApp(settings: new()
        {
            ["Dictation:Provider"] = "Http",
            ["Dictation:Http:Url"] = "http://stt.internal/inference",
            ["Dictation:Http:Mode"] = "Multipart",
            ["Dictation:Http:FileField"] = "audio",
            ["Dictation:Http:FormFields:language"] = "{{lang}}",
            ["Dictation:Http:FormFields:response_format"] = "json",
        });
        var api = new FakeApi((_, _) => Json(new { text = "hello period" }));
        using var f = With(app, api);
        var r = await Signed(f).PostAsync(Dictate("en-GB"), Wav());
        Assert.Equal("hello period", (await r.Content.ReadFromJsonAsync<JsonElement>()).GetProperty("text").GetString());
        Assert.True(api.Requests.TryDequeue(out var sent));
        Assert.StartsWith("multipart/form-data", sent.ContentType, StringComparison.Ordinal);
        var form = Encoding.Latin1.GetString(sent.Body);
        Assert.Contains("name=language", form, StringComparison.Ordinal);
        Assert.Contains("\r\n\r\nen\r\n", form, StringComparison.Ordinal);
        Assert.Contains("name=audio; filename=phrase.wav", form, StringComparison.Ordinal);
        Assert.Contains("RIFF", form, StringComparison.Ordinal);
    }

    [Fact]
    public async Task UsesARegisteredEngine()
    {
        using var app = new TestApp(settings: new() { ["Dictation:Provider"] = "Custom", ["Dictation:Languages:0"] = "vi" });
        var engine = new EchoEngine();
        using var f = app.WithWebHostBuilder(b => b.ConfigureServices(s => s.AddSingleton<IDictationEngine>(engine)));
        var client = Signed(f);
        var r = await client.PostAsync(Dictate(), Wav());
        Assert.Equal("xin chào chấm", (await r.Content.ReadFromJsonAsync<JsonElement>()).GetProperty("text").GetString());
        Assert.Equal([(Audio.Length, "audio/wav", "vi")], engine.Calls);
        // Only the configured languages.
        Assert.Equal(HttpStatusCode.BadRequest, (await client.PostAsync(Dictate("en-US"), Wav())).StatusCode);
    }

    [Fact]
    public async Task RefusesStrangersAndBadInput()
    {
        using var app = new TestApp(settings: new()
        {
            ["Dictation:Provider"] = "Http",
            ["Dictation:Http:Url"] = "http://stt.internal/recognize",
            ["Dictation:MaxAudioBytes"] = "100",
        });
        var api = new FakeApi((_, _) => Json(new { text = "x" }));
        using var f = With(app, api);
        var anon = f.CreateClient();
        Assert.Equal(HttpStatusCode.Unauthorized, (await anon.PostAsync(Dictate(), Wav())).StatusCode);
        Assert.Equal(HttpStatusCode.Unauthorized, (await anon.PostAsync(Dictate(session: "nope"), Wav())).StatusCode);
        Assert.Equal(HttpStatusCode.Unauthorized, (await anon.GetAsync(Languages)).StatusCode);
        var client = Signed(f);
        var r = await client.PostAsync(Dictate("fr-FR"), Wav());
        Assert.Equal(HttpStatusCode.BadRequest, r.StatusCode);
        Assert.Contains("unsupportedLanguage", await r.Content.ReadAsStringAsync(), StringComparison.Ordinal);
        Assert.Equal(HttpStatusCode.UnsupportedMediaType, (await client.PostAsync(Dictate(), Wav(type: "text/plain"))).StatusCode);
        r = await client.PostAsync(Dictate(), Wav(new byte[101]));
        Assert.Equal(HttpStatusCode.RequestEntityTooLarge, r.StatusCode);
        Assert.Contains("tooLarge", await r.Content.ReadAsStringAsync(), StringComparison.Ordinal);
        Assert.Equal(HttpStatusCode.BadRequest, (await client.PostAsync(Dictate(), Wav([]))).StatusCode);
        Assert.Empty(api.Requests);
    }

    [Fact]
    public async Task UpstreamFailuresAreReportedWithoutLoggingAudioOrText()
    {
        const string Secret = "Hợp đồng mật số 42";
        using var app = new TestApp(settings: new()
        {
            ["Dictation:Provider"] = "Http",
            ["Dictation:Http:Url"] = "http://stt.internal/recognize",
        });
        var status = HttpStatusCode.InternalServerError;
        var api = new FakeApi((_, _) => status == HttpStatusCode.OK ? Json(new { text = Secret }) : Json(new { error = "boom" }, status));
        var logs = new CaptureLogs();
        using var f = With(app, api, logs);
        var client = Signed(f);
        var r = await client.PostAsync(Dictate(), Wav());
        Assert.Equal(HttpStatusCode.BadGateway, r.StatusCode);
        Assert.Contains("upstream", await r.Content.ReadAsStringAsync(), StringComparison.Ordinal);
        status = HttpStatusCode.TooManyRequests;
        Assert.Equal(HttpStatusCode.TooManyRequests, (await client.PostAsync(Dictate(), Wav())).StatusCode);
        status = HttpStatusCode.OK;
        r = await client.PostAsync(Dictate(), Wav());
        Assert.Equal(Secret, (await r.Content.ReadFromJsonAsync<JsonElement>()).GetProperty("text").GetString());
        // The failure is logged (the capture works) but never the text or the audio.
        Assert.Contains(logs.Lines, l => l.Contains("HTTP 500", StringComparison.Ordinal));
        Assert.DoesNotContain(logs.Lines, l => l.Contains(Secret, StringComparison.Ordinal) || l.Contains("Hợp đồng", StringComparison.Ordinal) || l.Contains("RIFF", StringComparison.Ordinal));
    }

    [Fact]
    public void HttpProviderNeedsAUrl()
    {
        using var app = new TestApp(settings: new() { ["Dictation:Provider"] = "Http" });
        Assert.ThrowsAny<Exception>(() => app.CreateClient());
    }
}
