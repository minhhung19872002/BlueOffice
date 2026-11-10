using System.Security.Claims;
using BlueOffice.Api.Common;
using BlueOffice.Api.Sessions;
using BlueOffice.Api.Signing;

namespace BlueOffice.Api.WebData;

/// <summary>Options of the web page proxy of Get &amp; Transform data (<c>WebData</c> section).</summary>
public sealed class WebDataOptions
{
    /// <summary>Relay web pages at all. Default false: the endpoint answers 404 and nothing is fetched.</summary>
    public bool Enabled { get; set; }

    /// <summary>Allow pages on loopback, private and link-local addresses (an intranet). Default false.</summary>
    public bool AllowPrivateNetworks { get; set; }

    /// <summary>Largest page relayed, in bytes (default 20 MB).</summary>
    public long MaxBytes { get; set; } = 20 * 1024 * 1024;

    /// <summary>Upstream timeout (default 30 s).</summary>
    public TimeSpan Timeout { get; set; } = TimeSpan.FromSeconds(30);
}

/// <summary>Body of <c>POST /api/data/web</c>.</summary>
/// <param name="Url">The page (http or https).</param>
/// <param name="Session">Editor session id (with the <c>X-Session-Token</c> header) when the caller is not signed in.</param>
public sealed record WebDataRequest(string Url, string? Session);

/// <summary>
/// Fetches web pages for BlueSheet's "From Web" queries (SHEET-PQ-001), which the browser cannot read from other sites
/// itself (CORS). Off unless <c>WebData:Enabled</c>; signed-in users or editor sessions only; http/https GET without
/// credentials; by default no loopback, private or link-local address (checked when each connection is made, so DNS
/// rebinding and redirects cannot reach the internal network); answers are size-limited. See docs/deploy/web-data.md.
/// </summary>
public static class WebDataProxy
{
    /// <summary>Named HttpClient.</summary>
    public const string HttpClientName = "webdata";

    /// <summary>Registers the options and the HttpClient.</summary>
    public static IServiceCollection AddWebDataProxy(this IServiceCollection services, IConfiguration config)
    {
        var opts = new WebDataOptions();
        config.GetSection("WebData").Bind(opts);
        services.AddSingleton(opts);
        services.AddHttpClient(HttpClientName, c => c.Timeout = opts.Timeout)
            .ConfigurePrimaryHttpMessageHandler(() => new SocketsHttpHandler
            {
                AllowAutoRedirect = true,
                MaxAutomaticRedirections = 3,
                UseProxy = false,
                ConnectCallback = (ctx, ct) => RevocationProxy.ConnectAsync(ctx, opts.AllowPrivateNetworks, ct),
            });
        return services;
    }

    /// <summary>Maps <c>POST /api/data/web</c>.</summary>
    public static void MapWebDataProxy(this IEndpointRouteBuilder app)
    {
        app.MapPost("/api/data/web", FetchAsync).AllowAnonymous();
    }

    private static async Task<IResult> FetchAsync(
        WebDataRequest body,
        HttpRequest req,
        ClaimsPrincipal user,
        SessionStore sessions,
        TimeProvider time,
        IHttpClientFactory http,
        WebDataOptions opts,
        CancellationToken ct)
    {
        if (!opts.Enabled)
        {
            return Results.NotFound();
        }

        var signedIn = user.Identity?.IsAuthenticated == true;
        var inSession = body.Session is { Length: > 0 }
            && sessions.Get(body.Session, req.Headers[SessionEndpoints.TokenHeader], time.GetUtcNow().UtcDateTime) is not null;
        if (!signedIn && !inSession)
        {
            return Results.Unauthorized();
        }

        if (body.Url is null || body.Url.Length > 2048 || !Uri.TryCreate(body.Url, UriKind.Absolute, out var url)
            || (url.Scheme != Uri.UriSchemeHttp && url.Scheme != Uri.UriSchemeHttps) || !string.IsNullOrEmpty(url.UserInfo))
        {
            throw ApiException.BadRequest("url must be an http(s) URL.");
        }

        using var msg = new HttpRequestMessage(HttpMethod.Get, url);
        msg.Headers.Accept.ParseAdd("text/html, application/xhtml+xml, application/json, application/xml, text/csv, */*;q=0.5");
        HttpResponseMessage resp;
        try
        {
            resp = await http.CreateClient(HttpClientName).SendAsync(msg, HttpCompletionOption.ResponseHeadersRead, ct);
        }
        catch (HttpRequestException e)
        {
            return Results.Problem(e.Message, statusCode: StatusCodes.Status502BadGateway);
        }
        catch (TaskCanceledException) when (!ct.IsCancellationRequested)
        {
            return Results.Problem("upstream timeout", statusCode: StatusCodes.Status504GatewayTimeout);
        }

        using (resp)
        {
            if (!resp.IsSuccessStatusCode)
            {
                return Results.Problem($"upstream HTTP {(int)resp.StatusCode}", statusCode: StatusCodes.Status502BadGateway);
            }

            if (resp.Content.Headers.ContentLength > opts.MaxBytes)
            {
                return Results.Problem("page too large", statusCode: StatusCodes.Status413PayloadTooLarge);
            }

            await using var stream = await resp.Content.ReadAsStreamAsync(ct);
            using var buffer = new MemoryStream();
            var chunk = new byte[81920];
            int n;
            while ((n = await stream.ReadAsync(chunk, ct)) > 0)
            {
                if (buffer.Length + n > opts.MaxBytes)
                {
                    return Results.Problem("page too large", statusCode: StatusCodes.Status413PayloadTooLarge);
                }

                buffer.Write(chunk, 0, n);
            }

            return Results.Bytes(buffer.ToArray(), "application/octet-stream");
        }
    }
}
