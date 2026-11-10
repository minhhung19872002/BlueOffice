using System.Globalization;
using System.Net;
using System.Net.Http.Json;
using System.Security.Claims;
using System.Text.Json;
using System.Text.Json.Serialization;
using BlueOffice.Api.Sessions;
using BlueOffice.Api.Translation;
using Microsoft.Extensions.DependencyInjection.Extensions;

namespace BlueOffice.Api.Handwriting;

/// <summary>Options of the handwriting recognition proxy (<c>Handwriting</c> section; docs/deploy/handwriting.md).</summary>
public sealed class HandwritingOptions
{
    /// <summary><c>None</c> (default: off, no ink is sent anywhere), <c>Http</c> (the configured recognition HTTP
    /// service) or <c>Custom</c> (an <see cref="IHandwritingEngine"/> registered by the deployment, e.g. a local model).</summary>
    public string Provider { get; set; } = "None";

    /// <summary>What the service recognises: <c>text</c> and / or <c>math</c> (default <c>text</c>).</summary>
    public List<string> Modes { get; set; } = [];

    /// <summary>Language codes offered to the editors (primary subtags).</summary>
    public List<string> Languages { get; set; } = [];

    /// <summary>Notation of math answers: <c>latex</c> (default) or <c>unicodeMath</c>.</summary>
    public string MathFormat { get; set; } = "latex";

    /// <summary>Most strokes in one request.</summary>
    public int MaxStrokes { get; set; } = 2000;

    /// <summary>Most points (all strokes) in one request.</summary>
    public int MaxPoints { get; set; } = 200_000;

    /// <summary>Largest request body (bytes).</summary>
    public long MaxBodyBytes { get; set; } = 4 * 1024 * 1024;

    /// <summary>Upstream timeout.</summary>
    public TimeSpan Timeout { get; set; } = TimeSpan.FromSeconds(30);

    /// <summary>The HTTP service (<c>Provider = Http</c>).</summary>
    public HttpHandwritingOptions Http { get; set; } = new();

    /// <summary>Offered modes.</summary>
    public IReadOnlyList<string> EffectiveModes =>
        Modes.Count > 0
            ? Modes.Select(m => m.Trim().ToLowerInvariant()).Where(m => m is "text" or "math").Distinct().ToList()
            : ["text"];

    /// <summary>Offered languages (default Vietnamese and English).</summary>
    public IReadOnlyList<string> EffectiveLanguages => Languages.Count > 0 ? Languages : ["vi", "en"];

    /// <summary>The math notation as the editors spell it.</summary>
    public string EffectiveMathFormat => MathFormat.Equals("unicodeMath", StringComparison.OrdinalIgnoreCase) ? "unicodeMath" : "latex";
}

/// <summary>A recognition HTTP service: the strokes posted as JSON, the reading taken from the JSON answer through paths.</summary>
public sealed class HttpHandwritingOptions
{
    /// <summary>Endpoint URL; <c>{{mode}}</c> and <c>{{lang}}</c> are replaced (URL-encoded).</summary>
    public string Url { get; set; } = "";

    /// <summary>Extra request headers (authentication).</summary>
    public Dictionary<string, string> Headers { get; set; } = [];

    /// <summary>Where the best reading is in the answer (dot path as for translation: <c>text</c>, <c>results[0].text</c>).</summary>
    public string ResponsePath { get; set; } = "text";

    /// <summary>Where other readings are (an array of strings or several values), if the service gives them.</summary>
    public string AlternativesPath { get; set; } = "";

    /// <summary>Language codes as the service spells them (<c>{"vi": "vi-VN"}</c>); unmapped: the primary subtag.</summary>
    public Dictionary<string, string> LanguageMap { get; set; } = [];
}

/// <summary>One stroke: points <c>[x, y]</c> or <c>[x, y, t]</c> (slide points; t in ms).</summary>
/// <param name="Points">Samples in drawing order.</param>
public sealed record HandwritingStroke(List<double[]> Points);

/// <summary>What is recognised.</summary>
/// <param name="Mode"><c>text</c> or <c>math</c>.</param>
/// <param name="Lang">Primary subtag.</param>
/// <param name="Strokes">The ink.</param>
public sealed record HandwritingInput(string Mode, string Lang, IReadOnlyList<HandwritingStroke> Strokes);

/// <summary>A reading of the ink.</summary>
/// <param name="Text">Best reading (empty: nothing recognised).</param>
/// <param name="Alternatives">Other readings, best first.</param>
public sealed record HandwritingReading(string Text, IReadOnlyList<string> Alternatives);

/// <summary>A failure with the status and code returned to the editor.</summary>
public sealed class HandwritingException(int status, string code, string message) : Exception(message)
{
    /// <summary>HTTP status for the editor.</summary>
    public int Status { get; } = status;

    /// <summary>Error code (<c>upstream</c>, <c>rateLimited</c> …).</summary>
    public string Code { get; } = code;
}

/// <summary>Recognises handwriting. The HTTP service is built in; a deployment can register its own (a local model)
/// with <c>Handwriting:Provider = Custom</c>. Implementations must never log the ink or the reading.</summary>
public interface IHandwritingEngine
{
    /// <summary>Reading of <paramref name="input"/>.</summary>
    Task<HandwritingReading> RecognizeAsync(HandwritingInput input, CancellationToken ct);
}

/// <summary>The configured recognition HTTP service.</summary>
public sealed class HttpHandwritingEngine(IHttpClientFactory http, HandwritingOptions options, ILogger<HttpHandwritingEngine> logger) : IHandwritingEngine
{
    private const long MaxAnswerBytes = 1024 * 1024;

    private static HandwritingException Invalid() => new(StatusCodes.Status502BadGateway, "upstream", "Unexpected answer from the handwriting recognition service.");

    /// <summary>The request for <paramref name="input"/>: <c>{"mode", "language", "strokes": [{"points": [[x, y, t]]}]}</c>.</summary>
    public static HttpRequestMessage Request(HttpHandwritingOptions o, HandwritingInput input)
    {
        var lang = o.LanguageMap.TryGetValue(input.Lang, out var m) ? m : input.Lang;
        var url = o.Url
            .Replace("{{mode}}", Uri.EscapeDataString(input.Mode), StringComparison.Ordinal)
            .Replace("{{lang}}", Uri.EscapeDataString(lang), StringComparison.Ordinal);
        var msg = new HttpRequestMessage(HttpMethod.Post, url)
        {
            Content = JsonContent.Create(new { mode = input.Mode, language = lang, strokes = input.Strokes.Select(s => new { points = s.Points }) }),
        };
        foreach (var (k, v) in o.Headers)
        {
            msg.Headers.TryAddWithoutValidation(k, v);
        }

        return msg;
    }

    /// <inheritdoc />
    public async Task<HandwritingReading> RecognizeAsync(HandwritingInput input, CancellationToken ct)
    {
        using var msg = Request(options.Http, input);
        HttpResponseMessage resp;
        try
        {
            resp = await http.CreateClient(HandwritingEndpoints.HttpClientName).SendAsync(msg, HttpCompletionOption.ResponseHeadersRead, ct);
        }
        catch (HttpRequestException)
        {
            // Only the fact is logged: never the ink or the reading.
            logger.LogWarning("Handwriting recognition service unreachable");
            throw new HandwritingException(StatusCodes.Status502BadGateway, "upstream", "The handwriting recognition service is unreachable.");
        }
        catch (TaskCanceledException) when (!ct.IsCancellationRequested)
        {
            logger.LogWarning("Handwriting recognition service timed out");
            throw new HandwritingException(StatusCodes.Status504GatewayTimeout, "upstream", "The handwriting recognition service timed out.");
        }

        using (resp)
        {
            if (!resp.IsSuccessStatusCode)
            {
                logger.LogWarning("Handwriting recognition service answered HTTP {Status}", (int)resp.StatusCode);
                throw resp.StatusCode switch
                {
                    HttpStatusCode.TooManyRequests => new HandwritingException(StatusCodes.Status429TooManyRequests, "rateLimited", "The handwriting recognition service is busy."),
                    HttpStatusCode.RequestEntityTooLarge => new HandwritingException(StatusCodes.Status413PayloadTooLarge, "tooLarge", "Too much ink for the handwriting recognition service."),
                    _ => new HandwritingException(StatusCodes.Status502BadGateway, "upstream", $"The handwriting recognition service answered HTTP {(int)resp.StatusCode}."),
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
                static IEnumerable<string> Strings(JsonElement e) => e.ValueKind switch
                {
                    JsonValueKind.Array => e.EnumerateArray().Select(x => x.ValueKind == JsonValueKind.String ? x.GetString()! : throw Invalid()),
                    JsonValueKind.String => [e.GetString()!],
                    _ => throw Invalid(),
                };

                var best = HttpTranslationEngine.Select(doc.RootElement, options.Http.ResponsePath).SelectMany(Strings).FirstOrDefault() ?? throw Invalid();
                var alternatives = options.Http.AlternativesPath.Length > 0
                    ? HttpTranslationEngine.Select(doc.RootElement, options.Http.AlternativesPath).SelectMany(Strings)
                        .Select(a => a.Trim()).Where(a => a.Length > 0 && a != best.Trim()).Distinct().Take(10).ToList()
                    : [];
                return new HandwritingReading(best.Trim(), alternatives);
            }
        }
    }
}

/// <summary>Request body of <c>POST /api/handwriting</c>.</summary>
public sealed class HandwritingBody
{
    /// <summary>The ink.</summary>
    [JsonPropertyName("strokes")]
    public List<HandwritingStroke>? Strokes { get; set; }
}

/// <summary>
/// <c>GET /api/handwriting/capabilities</c> and <c>POST /api/handwriting</c> (CORE-INK-002): an editor sends ink
/// strokes and gets the recognised text or equation; the server forwards them to the recognition service an
/// administrator configured. Off by default (404). Neither the ink nor the reading is logged or stored.
/// </summary>
public static class HandwritingEndpoints
{
    /// <summary>Named HttpClient of the HTTP service.</summary>
    public const string HttpClientName = "handwriting";

    private static readonly JsonSerializerOptions BodyJson = new(JsonSerializerDefaults.Web) { MaxDepth = 8 };

    /// <summary>Registers the options, the HttpClient and the configured engine.</summary>
    public static IServiceCollection AddHandwriting(this IServiceCollection services, IConfiguration config)
    {
        var opts = new HandwritingOptions();
        config.GetSection("Handwriting").Bind(opts);
        services.AddSingleton(opts);
        services.AddHttpClient(HttpClientName, c => c.Timeout = opts.Timeout);
        if (opts.Provider.Equals("Http", StringComparison.OrdinalIgnoreCase))
        {
            var probe = opts.Http.Url.Replace("{{mode}}", "text", StringComparison.Ordinal).Replace("{{lang}}", "vi", StringComparison.Ordinal);
            if (!Uri.TryCreate(probe, UriKind.Absolute, out var u) || (u.Scheme != Uri.UriSchemeHttp && u.Scheme != Uri.UriSchemeHttps))
            {
                throw new InvalidOperationException("Handwriting:Http:Url must be an http(s) URL when Handwriting:Provider is Http.");
            }

            services.TryAddSingleton<IHandwritingEngine, HttpHandwritingEngine>();
        }

        return services;
    }

    /// <summary>Maps the endpoints.</summary>
    public static void MapHandwriting(this IEndpointRouteBuilder app)
    {
        app.MapGet("/api/handwriting/capabilities", Capabilities).AllowAnonymous();
        app.MapPost("/api/handwriting", RecognizeAsync).AllowAnonymous();
    }

    private static IHandwritingEngine? Engine(IServiceProvider sp, HandwritingOptions opts) =>
        opts.Provider.Equals("None", StringComparison.OrdinalIgnoreCase) || opts.EffectiveModes.Count == 0 ? null : sp.GetService<IHandwritingEngine>();

    private static bool Allowed(HttpRequest req, ClaimsPrincipal user, SessionStore sessions, TimeProvider time, string? session) =>
        user.Identity?.IsAuthenticated == true
        || (session is { Length: > 0 } && sessions.Get(session, req.Headers[SessionEndpoints.TokenHeader], time.GetUtcNow().UtcDateTime) is not null);

    private static IResult Problem(int status, string code, string detail) =>
        Results.Problem(detail, statusCode: status, extensions: new Dictionary<string, object?> { ["code"] = code });

    private static IResult Capabilities(HttpRequest req, ClaimsPrincipal user, SessionStore sessions, TimeProvider time, HandwritingOptions opts, IServiceProvider sp, string? session)
    {
        if (Engine(sp, opts) is null)
        {
            return Problem(StatusCodes.Status404NotFound, "unavailable", "Handwriting recognition is not configured.");
        }

        return Allowed(req, user, sessions, time, session)
            ? Results.Ok(new { modes = opts.EffectiveModes, languages = opts.EffectiveLanguages, mathFormat = opts.EffectiveMathFormat })
            : Results.Unauthorized();
    }

    private static async Task<IResult> RecognizeAsync(
        HttpRequest req,
        ClaimsPrincipal user,
        SessionStore sessions,
        TimeProvider time,
        HandwritingOptions opts,
        IServiceProvider sp,
        string? mode,
        string? lang,
        string? session,
        CancellationToken ct)
    {
        var engine = Engine(sp, opts);
        if (engine is null)
        {
            return Problem(StatusCodes.Status404NotFound, "unavailable", "Handwriting recognition is not configured.");
        }

        if (!Allowed(req, user, sessions, time, session))
        {
            return Results.Unauthorized();
        }

        var m = (mode ?? "text").Trim().ToLowerInvariant();
        if (!opts.EffectiveModes.Contains(m))
        {
            return Problem(StatusCodes.Status400BadRequest, "unsupportedMode", "Mode not offered.");
        }

        var primary = (lang ?? "").Trim().Split('-', '_')[0].ToLower(CultureInfo.InvariantCulture);
        if (primary.Length == 0 || !opts.EffectiveLanguages.Contains(primary, StringComparer.OrdinalIgnoreCase))
        {
            return Problem(StatusCodes.Status400BadRequest, "unsupportedLanguage", "Language not offered.");
        }

        if (!(req.ContentType ?? "").StartsWith("application/json", StringComparison.OrdinalIgnoreCase))
        {
            return Problem(StatusCodes.Status415UnsupportedMediaType, "invalid", "The body must be JSON.");
        }

        if (req.ContentLength > opts.MaxBodyBytes)
        {
            return Problem(StatusCodes.Status413PayloadTooLarge, "tooLarge", $"At most {opts.MaxBodyBytes} bytes of ink.");
        }

        using var buf = new MemoryStream();
        var chunk = new byte[64 * 1024];
        int n;
        while ((n = await req.Body.ReadAsync(chunk, ct)) > 0)
        {
            if (buf.Length + n > opts.MaxBodyBytes)
            {
                return Problem(StatusCodes.Status413PayloadTooLarge, "tooLarge", $"At most {opts.MaxBodyBytes} bytes of ink.");
            }

            buf.Write(chunk, 0, n);
        }

        HandwritingBody? body;
        try
        {
            body = JsonSerializer.Deserialize<HandwritingBody>(buf.GetBuffer().AsSpan(0, (int)buf.Length), BodyJson);
        }
        catch (JsonException)
        {
            return Problem(StatusCodes.Status400BadRequest, "invalid", "The body is not valid ink JSON.");
        }

        var strokes = body?.Strokes?.Where(s => s.Points is { Count: > 0 }).ToList() ?? [];
        if (strokes.Count == 0)
        {
            return Problem(StatusCodes.Status400BadRequest, "invalid", "No ink.");
        }

        if (strokes.Count > opts.MaxStrokes || strokes.Sum(s => s.Points.Count) > opts.MaxPoints)
        {
            return Problem(StatusCodes.Status413PayloadTooLarge, "tooLarge", "Too much ink.");
        }

        if (strokes.Any(s => s.Points.Any(p => p is null || p.Length is < 2 or > 3 || p.Any(v => !double.IsFinite(v)))))
        {
            return Problem(StatusCodes.Status400BadRequest, "invalid", "Points are [x, y] or [x, y, t] numbers.");
        }

        try
        {
            var r = await engine.RecognizeAsync(new HandwritingInput(m, primary, strokes), ct);
            object answer = m == "math"
                ? new { text = r.Text, alternatives = r.Alternatives, format = opts.EffectiveMathFormat }
                : new { text = r.Text, alternatives = r.Alternatives };
            return Results.Ok(answer);
        }
        catch (HandwritingException e)
        {
            return Problem(e.Status, e.Code, e.Message);
        }
    }
}
