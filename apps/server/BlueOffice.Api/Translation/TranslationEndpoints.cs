using System.Globalization;
using System.Net;
using System.Security.Claims;
using System.Text;
using System.Text.Json;
using BlueOffice.Api.Sessions;
using Microsoft.Extensions.DependencyInjection.Extensions;

namespace BlueOffice.Api.Translation;

/// <summary>Options of the translation proxy (<c>Translation</c> section; docs/deploy/translation.md).</summary>
public sealed class TranslationOptions
{
    /// <summary><c>None</c> (default: translation is off, nothing is sent anywhere), <c>Http</c> (the configured HTTP API)
    /// or <c>Custom</c> (an <see cref="ITranslationEngine"/> registered by the deployment, e.g. a local model).</summary>
    public string Provider { get; set; } = "None";

    /// <summary>Language codes offered to the editors (primary subtags).</summary>
    public List<string> Languages { get; set; } = [];

    /// <summary>Most texts per request.</summary>
    public int MaxTexts { get; set; } = 128;

    /// <summary>Most characters per request (all texts).</summary>
    public int MaxChars { get; set; } = 20000;

    /// <summary>Upstream timeout.</summary>
    public TimeSpan Timeout { get; set; } = TimeSpan.FromSeconds(30);

    /// <summary>The HTTP API (<c>Provider = Http</c>).</summary>
    public HttpTranslationOptions Http { get; set; } = new();

    /// <summary>Offered languages (default Vietnamese and English).</summary>
    public IReadOnlyList<string> EffectiveLanguages => Languages.Count > 0 ? Languages : ["vi", "en"];
}

/// <summary>A generic HTTP translation API: request built from a JSON template, answer read through a path.</summary>
public sealed class HttpTranslationOptions
{
    /// <summary>Endpoint URL.</summary>
    public string Url { get; set; } = "";

    /// <summary>Extra request headers (authentication: <c>Authorization</c>, an API key header …).</summary>
    public Dictionary<string, string> Headers { get; set; } = [];

    /// <summary>JSON body; <c>{{texts}}</c> (JSON array), <c>{{text}}</c> (JSON string, with <see cref="PerText"/>),
    /// <c>{{from}}</c> and <c>{{to}}</c> (JSON strings) are replaced.</summary>
    public string BodyTemplate { get; set; } = "{\"q\":{{texts}},\"source\":{{from}},\"target\":{{to}},\"format\":\"text\"}";

    /// <summary>One upstream request per text (APIs that take a single string).</summary>
    public bool PerText { get; set; }

    /// <summary>Where the translations are in the answer: dot path, <c>[*]</c> maps over an array, <c>[n]</c> indexes
    /// (<c>translatedText</c>, <c>translations[*].text</c>, <c>data.translations[*].translatedText</c>).</summary>
    public string ResponsePath { get; set; } = "translatedText";

    /// <summary>Value sent for an undetected source language.</summary>
    public string AutoSource { get; set; } = "auto";

    /// <summary>Language codes as the API spells them (<c>{"en": "EN-US"}</c>).</summary>
    public Dictionary<string, string> LanguageMap { get; set; } = [];
}

/// <summary>A translation failure with the status and code returned to the editor.</summary>
public sealed class TranslationException(int status, string code, string message) : Exception(message)
{
    /// <summary>HTTP status for the editor.</summary>
    public int Status { get; } = status;

    /// <summary>Error code (<c>upstream</c>, <c>unsupportedLanguage</c> …).</summary>
    public string Code { get; } = code;
}

/// <summary>Translates texts. The HTTP API is built in; a deployment can register its own (a local model) with
/// <c>Translation:Provider = Custom</c>. Implementations must never log the texts.</summary>
public interface ITranslationEngine
{
    /// <summary>Translations of <paramref name="texts"/>, same order and count.</summary>
    Task<IReadOnlyList<string>> TranslateAsync(IReadOnlyList<string> texts, string from, string to, CancellationToken ct);
}

/// <summary>The configured HTTP translation API.</summary>
public sealed class HttpTranslationEngine(IHttpClientFactory http, TranslationOptions options, ILogger<HttpTranslationEngine> logger) : ITranslationEngine
{
    private const long MaxAnswerBytes = 8 * 1024 * 1024;

    /// <inheritdoc />
    public async Task<IReadOnlyList<string>> TranslateAsync(IReadOnlyList<string> texts, string from, string to, CancellationToken ct)
    {
        var o = options.Http;
        if (!o.PerText)
        {
            var got = await SendAsync(Body(o, JsonSerializer.Serialize(texts), null, from, to), ct);
            return got.Count == texts.Count ? got : throw Invalid();
        }

        var outp = new List<string>(texts.Count);
        foreach (var t in texts)
        {
            var got = await SendAsync(Body(o, JsonSerializer.Serialize(new[] { t }), t, from, to), ct);
            outp.Add(got.Count == 1 ? got[0] : throw Invalid());
        }

        return outp;
    }

    private static TranslationException Invalid() => new(StatusCodes.Status502BadGateway, "upstream", "Unexpected answer from the translation service.");

    private static string Lang(HttpTranslationOptions o, string code) =>
        code == "auto" ? o.AutoSource : o.LanguageMap.TryGetValue(code, out var m) ? m : code;

    /// <summary>The request body from the template (values JSON-encoded).</summary>
    public static string Body(HttpTranslationOptions o, string textsJson, string? text, string from, string to) =>
        o.BodyTemplate
            .Replace("{{texts}}", textsJson, StringComparison.Ordinal)
            .Replace("{{text}}", JsonSerializer.Serialize(text ?? ""), StringComparison.Ordinal)
            .Replace("{{from}}", JsonSerializer.Serialize(Lang(o, from)), StringComparison.Ordinal)
            .Replace("{{to}}", JsonSerializer.Serialize(Lang(o, to)), StringComparison.Ordinal);

    private async Task<List<string>> SendAsync(string body, CancellationToken ct)
    {
        using var msg = new HttpRequestMessage(HttpMethod.Post, options.Http.Url)
        {
            Content = new StringContent(body, Encoding.UTF8, "application/json"),
        };
        foreach (var (k, v) in options.Http.Headers)
        {
            msg.Headers.TryAddWithoutValidation(k, v);
        }

        HttpResponseMessage resp;
        try
        {
            resp = await http.CreateClient(TranslationEndpoints.HttpClientName).SendAsync(msg, HttpCompletionOption.ResponseHeadersRead, ct);
        }
        catch (HttpRequestException)
        {
            // Only the fact is logged: never the texts.
            logger.LogWarning("Translation service unreachable");
            throw new TranslationException(StatusCodes.Status502BadGateway, "upstream", "The translation service is unreachable.");
        }
        catch (TaskCanceledException) when (!ct.IsCancellationRequested)
        {
            logger.LogWarning("Translation service timed out");
            throw new TranslationException(StatusCodes.Status504GatewayTimeout, "upstream", "The translation service timed out.");
        }

        using (resp)
        {
            if (!resp.IsSuccessStatusCode)
            {
                logger.LogWarning("Translation service answered HTTP {Status}", (int)resp.StatusCode);
                throw resp.StatusCode switch
                {
                    HttpStatusCode.TooManyRequests => new TranslationException(StatusCodes.Status429TooManyRequests, "rateLimited", "The translation service is busy."),
                    HttpStatusCode.RequestEntityTooLarge => new TranslationException(StatusCodes.Status413PayloadTooLarge, "tooLarge", "Too much text for the translation service."),
                    _ => new TranslationException(StatusCodes.Status502BadGateway, "upstream", $"The translation service answered HTTP {(int)resp.StatusCode}."),
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
                var found = Select(doc.RootElement, options.Http.ResponsePath);
                var outp = new List<string>();
                foreach (var e in found)
                {
                    if (e.ValueKind == JsonValueKind.Array)
                    {
                        outp.AddRange(e.EnumerateArray().Select(x => x.ValueKind == JsonValueKind.String ? x.GetString()! : throw Invalid()));
                    }
                    else
                    {
                        outp.Add(e.ValueKind == JsonValueKind.String ? e.GetString()! : throw Invalid());
                    }
                }

                return outp;
            }
        }
    }

    /// <summary>Elements at <paramref name="path"/> (dot path, <c>[*]</c> over arrays, <c>[n]</c> index; empty = root).</summary>
    public static List<JsonElement> Select(JsonElement root, string path)
    {
        var cur = new List<JsonElement> { root };
        foreach (var raw in path.Split('.', StringSplitOptions.RemoveEmptyEntries))
        {
            var name = raw;
            var index = (string?)null;
            var b = raw.IndexOf('[', StringComparison.Ordinal);
            if (b >= 0 && raw.EndsWith(']'))
            {
                name = raw[..b];
                index = raw[(b + 1)..^1];
            }

            var next = new List<JsonElement>();
            foreach (var e in cur)
            {
                var v = e;
                if (name.Length > 0)
                {
                    if (e.ValueKind != JsonValueKind.Object || !e.TryGetProperty(name, out v))
                    {
                        continue;
                    }
                }

                if (index is null)
                {
                    next.Add(v);
                }
                else if (v.ValueKind == JsonValueKind.Array)
                {
                    if (index == "*")
                    {
                        next.AddRange(v.EnumerateArray());
                    }
                    else if (int.TryParse(index, NumberStyles.None, CultureInfo.InvariantCulture, out var i) && i < v.GetArrayLength())
                    {
                        next.Add(v[i]);
                    }
                }
            }

            cur = next;
        }

        return cur;
    }
}

/// <summary>Body of <c>POST /api/translate</c>.</summary>
/// <param name="Texts">Texts to translate.</param>
/// <param name="From">Source language code or <c>auto</c>.</param>
/// <param name="To">Target language code.</param>
/// <param name="Session">Editor session id (with the <c>X-Session-Token</c> header) when the caller is not signed in.</param>
public sealed record TranslateRequest(IReadOnlyList<string>? Texts, string? From, string? To, string? Session);

/// <summary>
/// <c>POST /api/translate</c> and <c>GET /api/translate/languages</c> (CORE-LANG-001): the editors' translation goes
/// through the server, which forwards it to the service an administrator configured. Off by default (404). The texts
/// are never logged or stored.
/// </summary>
public static class TranslationEndpoints
{
    /// <summary>Named HttpClient of the HTTP API.</summary>
    public const string HttpClientName = "translation";

    /// <summary>Registers the options, the HttpClient and the configured engine.</summary>
    public static IServiceCollection AddTranslation(this IServiceCollection services, IConfiguration config)
    {
        var opts = new TranslationOptions();
        config.GetSection("Translation").Bind(opts);
        services.AddSingleton(opts);
        services.AddHttpClient(HttpClientName, c => c.Timeout = opts.Timeout);
        if (opts.Provider.Equals("Http", StringComparison.OrdinalIgnoreCase))
        {
            if (!Uri.TryCreate(opts.Http.Url, UriKind.Absolute, out var u) || (u.Scheme != Uri.UriSchemeHttp && u.Scheme != Uri.UriSchemeHttps))
            {
                throw new InvalidOperationException("Translation:Http:Url must be an http(s) URL when Translation:Provider is Http.");
            }

            services.TryAddSingleton<ITranslationEngine, HttpTranslationEngine>();
        }

        return services;
    }

    /// <summary>Maps the endpoints.</summary>
    public static void MapTranslation(this IEndpointRouteBuilder app)
    {
        app.MapGet("/api/translate/languages", Languages).AllowAnonymous();
        app.MapPost("/api/translate", TranslateAsync).AllowAnonymous();
    }

    private static ITranslationEngine? Engine(IServiceProvider sp, TranslationOptions opts) =>
        opts.Provider.Equals("None", StringComparison.OrdinalIgnoreCase) ? null : sp.GetService<ITranslationEngine>();

    private static bool Allowed(HttpRequest req, ClaimsPrincipal user, SessionStore sessions, TimeProvider time, string? session) =>
        user.Identity?.IsAuthenticated == true
        || (session is { Length: > 0 } && sessions.Get(session, req.Headers[SessionEndpoints.TokenHeader], time.GetUtcNow().UtcDateTime) is not null);

    private static IResult Problem(int status, string code, string detail) =>
        Results.Problem(detail, statusCode: status, extensions: new Dictionary<string, object?> { ["code"] = code });

    private static IResult Languages(HttpRequest req, ClaimsPrincipal user, SessionStore sessions, TimeProvider time, TranslationOptions opts, IServiceProvider sp, string? session)
    {
        if (Engine(sp, opts) is null)
        {
            return Problem(StatusCodes.Status404NotFound, "unavailable", "Translation is not configured.");
        }

        return Allowed(req, user, sessions, time, session)
            ? Results.Ok(new { languages = opts.EffectiveLanguages })
            : Results.Unauthorized();
    }

    private static async Task<IResult> TranslateAsync(
        TranslateRequest body,
        HttpRequest req,
        ClaimsPrincipal user,
        SessionStore sessions,
        TimeProvider time,
        TranslationOptions opts,
        IServiceProvider sp,
        CancellationToken ct)
    {
        var engine = Engine(sp, opts);
        if (engine is null)
        {
            return Problem(StatusCodes.Status404NotFound, "unavailable", "Translation is not configured.");
        }

        if (!Allowed(req, user, sessions, time, body.Session))
        {
            return Results.Unauthorized();
        }

        var texts = body.Texts ?? [];
        var from = (body.From ?? "auto").Trim().ToLowerInvariant();
        var to = (body.To ?? "").Trim().ToLowerInvariant();
        var langs = opts.EffectiveLanguages;
        if (texts.Count == 0 || texts.Any(t => t is null))
        {
            return Problem(StatusCodes.Status400BadRequest, "invalid", "texts must be a non-empty list of strings.");
        }

        if (!langs.Contains(to, StringComparer.OrdinalIgnoreCase) || (from != "auto" && !langs.Contains(from, StringComparer.OrdinalIgnoreCase)))
        {
            return Problem(StatusCodes.Status400BadRequest, "unsupportedLanguage", "Language not offered.");
        }

        if (from == to)
        {
            return Problem(StatusCodes.Status400BadRequest, "sameLanguage", "Source and target are the same language.");
        }

        if (texts.Count > opts.MaxTexts || texts.Sum(t => (long)t.Length) > opts.MaxChars)
        {
            return Problem(StatusCodes.Status413PayloadTooLarge, "tooLarge", $"At most {opts.MaxTexts} texts and {opts.MaxChars} characters per request.");
        }

        try
        {
            var outp = await engine.TranslateAsync(texts, from, to, ct);
            if (outp.Count != texts.Count)
            {
                return Problem(StatusCodes.Status502BadGateway, "upstream", "Unexpected answer from the translation service.");
            }

            return Results.Ok(new { texts = outp });
        }
        catch (TranslationException e)
        {
            return Problem(e.Status, e.Code, e.Message);
        }
    }
}
