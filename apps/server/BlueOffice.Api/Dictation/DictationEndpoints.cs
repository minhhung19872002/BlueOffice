using System.Globalization;
using System.Net;
using System.Net.Http.Headers;
using System.Security.Claims;
using System.Text.Json;
using BlueOffice.Api.Sessions;
using BlueOffice.Api.Translation;
using Microsoft.Extensions.DependencyInjection.Extensions;

namespace BlueOffice.Api.Dictation;

/// <summary>Options of the dictation proxy (<c>Dictation</c> section; docs/deploy/dictation.md).</summary>
public sealed class DictationOptions
{
    /// <summary><c>None</c> (default: dictation through the server is off, no audio is sent anywhere), <c>Http</c>
    /// (the configured speech-to-text HTTP service) or <c>Custom</c> (an <see cref="IDictationEngine"/> registered by
    /// the deployment, e.g. a local model).</summary>
    public string Provider { get; set; } = "None";

    /// <summary>Language codes offered to the editors (primary subtags).</summary>
    public List<string> Languages { get; set; } = [];

    /// <summary>Largest audio of one phrase (bytes; 16 kHz 16-bit mono WAV is 32 KB per second).</summary>
    public long MaxAudioBytes { get; set; } = 4 * 1024 * 1024;

    /// <summary>Upstream timeout.</summary>
    public TimeSpan Timeout { get; set; } = TimeSpan.FromSeconds(60);

    /// <summary>The HTTP service (<c>Provider = Http</c>).</summary>
    public HttpDictationOptions Http { get; set; } = new();

    /// <summary>Offered languages (default Vietnamese and English).</summary>
    public IReadOnlyList<string> EffectiveLanguages => Languages.Count > 0 ? Languages : ["vi", "en"];
}

/// <summary>A generic speech-to-text HTTP service: the phrase's audio posted raw or as a multipart form, the text
/// read from the JSON answer through a path.</summary>
public sealed class HttpDictationOptions
{
    /// <summary>Endpoint URL; <c>{{lang}}</c> is replaced by the (mapped, URL-encoded) language.</summary>
    public string Url { get; set; } = "";

    /// <summary>Extra request headers (authentication: <c>Authorization</c>, an API key header …).</summary>
    public Dictionary<string, string> Headers { get; set; } = [];

    /// <summary><c>Raw</c> (the audio is the request body, with its content type) or <c>Multipart</c>
    /// (<c>multipart/form-data</c> with the audio in <see cref="FileField"/> and <see cref="FormFields"/>).</summary>
    public string Mode { get; set; } = "Raw";

    /// <summary>Form field of the audio file (<c>Multipart</c>).</summary>
    public string FileField { get; set; } = "file";

    /// <summary>Other form fields (<c>Multipart</c>); <c>{{lang}}</c> in a value is replaced by the language.</summary>
    public Dictionary<string, string> FormFields { get; set; } = [];

    /// <summary>Where the text is in the answer (dot path as for translation: <c>text</c>,
    /// <c>results[0].alternatives[0].transcript</c>); several values are joined with spaces.</summary>
    public string ResponsePath { get; set; } = "text";

    /// <summary>Where the confidence (0–1) is, if the service reports one.</summary>
    public string ConfidencePath { get; set; } = "";

    /// <summary>Language codes as the service spells them (<c>{"vi": "vi-VN"}</c>); unmapped: the primary subtag.</summary>
    public Dictionary<string, string> LanguageMap { get; set; } = [];
}

/// <summary>Text of one phrase.</summary>
/// <param name="Text">Recognised text (empty: nothing recognised).</param>
/// <param name="Confidence">0–1 when known.</param>
public sealed record DictationTranscript(string Text, double? Confidence);

/// <summary>A dictation failure with the status and code returned to the editor.</summary>
public sealed class DictationException(int status, string code, string message) : Exception(message)
{
    /// <summary>HTTP status for the editor.</summary>
    public int Status { get; } = status;

    /// <summary>Error code (<c>upstream</c>, <c>rateLimited</c> …).</summary>
    public string Code { get; } = code;
}

/// <summary>Recognises the speech of one phrase. The HTTP service is built in; a deployment can register its own
/// (a local model) with <c>Dictation:Provider = Custom</c>. Implementations must never log the audio or the text.</summary>
public interface IDictationEngine
{
    /// <summary>Text of <paramref name="audio"/> (<paramref name="contentType"/>, e.g. <c>audio/wav</c>) spoken in
    /// <paramref name="lang"/> (primary subtag).</summary>
    Task<DictationTranscript> TranscribeAsync(ReadOnlyMemory<byte> audio, string contentType, string lang, CancellationToken ct);
}

/// <summary>The configured speech-to-text HTTP service.</summary>
public sealed class HttpDictationEngine(IHttpClientFactory http, DictationOptions options, ILogger<HttpDictationEngine> logger) : IDictationEngine
{
    private const long MaxAnswerBytes = 1024 * 1024;

    private static DictationException Invalid() => new(StatusCodes.Status502BadGateway, "upstream", "Unexpected answer from the speech recognition service.");

    /// <summary>The language as the service spells it.</summary>
    public static string Lang(HttpDictationOptions o, string lang) => o.LanguageMap.TryGetValue(lang, out var m) ? m : lang;

    /// <summary>The request for one phrase.</summary>
    public static HttpRequestMessage Request(HttpDictationOptions o, ReadOnlyMemory<byte> audio, string contentType, string lang)
    {
        var l = Lang(o, lang);
        var msg = new HttpRequestMessage(HttpMethod.Post, o.Url.Replace("{{lang}}", Uri.EscapeDataString(l), StringComparison.Ordinal));
        var file = new ReadOnlyMemoryContent(audio);
        file.Headers.ContentType = MediaTypeHeaderValue.Parse(contentType);
        if (o.Mode.Equals("Multipart", StringComparison.OrdinalIgnoreCase))
        {
            var form = new MultipartFormDataContent();
            foreach (var (k, v) in o.FormFields)
            {
                form.Add(new StringContent(v.Replace("{{lang}}", l, StringComparison.Ordinal)), k);
            }

            form.Add(file, o.FileField, contentType.Contains("wav", StringComparison.OrdinalIgnoreCase) ? "phrase.wav" : "phrase.audio");
            msg.Content = form;
        }
        else
        {
            msg.Content = file;
        }

        foreach (var (k, v) in o.Headers)
        {
            msg.Headers.TryAddWithoutValidation(k, v);
        }

        return msg;
    }

    /// <inheritdoc />
    public async Task<DictationTranscript> TranscribeAsync(ReadOnlyMemory<byte> audio, string contentType, string lang, CancellationToken ct)
    {
        using var msg = Request(options.Http, audio, contentType, lang);
        HttpResponseMessage resp;
        try
        {
            resp = await http.CreateClient(DictationEndpoints.HttpClientName).SendAsync(msg, HttpCompletionOption.ResponseHeadersRead, ct);
        }
        catch (HttpRequestException)
        {
            // Only the fact is logged: never the audio or the text.
            logger.LogWarning("Speech recognition service unreachable");
            throw new DictationException(StatusCodes.Status502BadGateway, "upstream", "The speech recognition service is unreachable.");
        }
        catch (TaskCanceledException) when (!ct.IsCancellationRequested)
        {
            logger.LogWarning("Speech recognition service timed out");
            throw new DictationException(StatusCodes.Status504GatewayTimeout, "upstream", "The speech recognition service timed out.");
        }

        using (resp)
        {
            if (!resp.IsSuccessStatusCode)
            {
                logger.LogWarning("Speech recognition service answered HTTP {Status}", (int)resp.StatusCode);
                throw resp.StatusCode switch
                {
                    HttpStatusCode.TooManyRequests => new DictationException(StatusCodes.Status429TooManyRequests, "rateLimited", "The speech recognition service is busy."),
                    HttpStatusCode.RequestEntityTooLarge => new DictationException(StatusCodes.Status413PayloadTooLarge, "tooLarge", "The phrase is too long for the speech recognition service."),
                    _ => new DictationException(StatusCodes.Status502BadGateway, "upstream", $"The speech recognition service answered HTTP {(int)resp.StatusCode}."),
                };
            }

            if (resp.Content.Headers.ContentLength > MaxAnswerBytes)
            {
                throw Invalid();
            }

            await using var stream = await resp.Content.ReadAsStreamAsync(ct);
            JsonDocument doc;
            try
            {
                doc = await JsonDocument.ParseAsync(stream, cancellationToken: ct);
            }
            catch (JsonException)
            {
                throw Invalid();
            }

            using (doc)
            {
                var parts = new List<string>();
                foreach (var e in HttpTranslationEngine.Select(doc.RootElement, options.Http.ResponsePath))
                {
                    if (e.ValueKind == JsonValueKind.Array)
                    {
                        parts.AddRange(e.EnumerateArray().Select(x => x.ValueKind == JsonValueKind.String ? x.GetString()! : throw Invalid()));
                    }
                    else
                    {
                        parts.Add(e.ValueKind == JsonValueKind.String ? e.GetString()! : throw Invalid());
                    }
                }

                double? confidence = null;
                if (options.Http.ConfidencePath.Length > 0)
                {
                    var c = HttpTranslationEngine.Select(doc.RootElement, options.Http.ConfidencePath).FirstOrDefault();
                    if (c.ValueKind == JsonValueKind.Number)
                    {
                        confidence = Math.Clamp(c.GetDouble(), 0, 1);
                    }
                }

                return new DictationTranscript(string.Join(' ', parts.Select(p => p.Trim()).Where(p => p.Length > 0)), confidence);
            }
        }
    }
}

/// <summary>
/// <c>POST /api/dictation</c> and <c>GET /api/dictation/languages</c> (CORE-LANG-002): an editor sends the audio of
/// one phrase (WAV) and gets its text; the server forwards it to the speech-to-text service an administrator
/// configured. Off by default (404). Neither the audio nor the text is logged or stored.
/// </summary>
public static class DictationEndpoints
{
    /// <summary>Named HttpClient of the HTTP service.</summary>
    public const string HttpClientName = "dictation";

    /// <summary>Registers the options, the HttpClient and the configured engine.</summary>
    public static IServiceCollection AddDictation(this IServiceCollection services, IConfiguration config)
    {
        var opts = new DictationOptions();
        config.GetSection("Dictation").Bind(opts);
        services.AddSingleton(opts);
        services.AddHttpClient(HttpClientName, c => c.Timeout = opts.Timeout);
        if (opts.Provider.Equals("Http", StringComparison.OrdinalIgnoreCase))
        {
            if (!Uri.TryCreate(opts.Http.Url.Replace("{{lang}}", "vi", StringComparison.Ordinal), UriKind.Absolute, out var u)
                || (u.Scheme != Uri.UriSchemeHttp && u.Scheme != Uri.UriSchemeHttps))
            {
                throw new InvalidOperationException("Dictation:Http:Url must be an http(s) URL when Dictation:Provider is Http.");
            }

            services.TryAddSingleton<IDictationEngine, HttpDictationEngine>();
        }

        return services;
    }

    /// <summary>Maps the endpoints.</summary>
    public static void MapDictation(this IEndpointRouteBuilder app)
    {
        app.MapGet("/api/dictation/languages", Languages).AllowAnonymous();
        app.MapPost("/api/dictation", TranscribeAsync).AllowAnonymous();
    }

    private static IDictationEngine? Engine(IServiceProvider sp, DictationOptions opts) =>
        opts.Provider.Equals("None", StringComparison.OrdinalIgnoreCase) ? null : sp.GetService<IDictationEngine>();

    private static bool Allowed(HttpRequest req, ClaimsPrincipal user, SessionStore sessions, TimeProvider time, string? session) =>
        user.Identity?.IsAuthenticated == true
        || (session is { Length: > 0 } && sessions.Get(session, req.Headers[SessionEndpoints.TokenHeader], time.GetUtcNow().UtcDateTime) is not null);

    private static IResult Problem(int status, string code, string detail) =>
        Results.Problem(detail, statusCode: status, extensions: new Dictionary<string, object?> { ["code"] = code });

    private static IResult Languages(HttpRequest req, ClaimsPrincipal user, SessionStore sessions, TimeProvider time, DictationOptions opts, IServiceProvider sp, string? session)
    {
        if (Engine(sp, opts) is null)
        {
            return Problem(StatusCodes.Status404NotFound, "unavailable", "Dictation is not configured.");
        }

        return Allowed(req, user, sessions, time, session)
            ? Results.Ok(new { languages = opts.EffectiveLanguages })
            : Results.Unauthorized();
    }

    private static async Task<IResult> TranscribeAsync(
        HttpRequest req,
        ClaimsPrincipal user,
        SessionStore sessions,
        TimeProvider time,
        DictationOptions opts,
        IServiceProvider sp,
        string? lang,
        string? session,
        CancellationToken ct)
    {
        var engine = Engine(sp, opts);
        if (engine is null)
        {
            return Problem(StatusCodes.Status404NotFound, "unavailable", "Dictation is not configured.");
        }

        if (!Allowed(req, user, sessions, time, session))
        {
            return Results.Unauthorized();
        }

        var primary = (lang ?? "").Trim().Split('-', '_')[0].ToLower(CultureInfo.InvariantCulture);
        if (primary.Length == 0 || !opts.EffectiveLanguages.Contains(primary, StringComparer.OrdinalIgnoreCase))
        {
            return Problem(StatusCodes.Status400BadRequest, "unsupportedLanguage", "Language not offered.");
        }

        var contentType = req.ContentType ?? "";
        if (!contentType.StartsWith("audio/", StringComparison.OrdinalIgnoreCase))
        {
            return Problem(StatusCodes.Status415UnsupportedMediaType, "invalid", "The body must be audio (audio/wav).");
        }

        if (req.ContentLength > opts.MaxAudioBytes)
        {
            return Problem(StatusCodes.Status413PayloadTooLarge, "tooLarge", $"At most {opts.MaxAudioBytes} bytes of audio per phrase.");
        }

        using var buf = new MemoryStream();
        var chunk = new byte[64 * 1024];
        int n;
        while ((n = await req.Body.ReadAsync(chunk, ct)) > 0)
        {
            if (buf.Length + n > opts.MaxAudioBytes)
            {
                return Problem(StatusCodes.Status413PayloadTooLarge, "tooLarge", $"At most {opts.MaxAudioBytes} bytes of audio per phrase.");
            }

            buf.Write(chunk, 0, n);
        }

        if (buf.Length == 0)
        {
            return Problem(StatusCodes.Status400BadRequest, "invalid", "No audio.");
        }

        try
        {
            var r = await engine.TranscribeAsync(buf.GetBuffer().AsMemory(0, (int)buf.Length), contentType, primary, ct);
            return r.Confidence is { } c ? Results.Ok(new { text = r.Text, confidence = c }) : Results.Ok(new { text = r.Text });
        }
        catch (DictationException e)
        {
            return Problem(e.Status, e.Code, e.Message);
        }
    }
}
