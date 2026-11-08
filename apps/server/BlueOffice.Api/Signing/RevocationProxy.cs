using System.Net;
using System.Net.Sockets;
using System.Security.Claims;
using BlueOffice.Api.Common;
using BlueOffice.Api.Sessions;

namespace BlueOffice.Api.Signing;

/// <summary>Options of the revocation proxy (<c>Revocation</c> section).</summary>
public sealed class RevocationOptions
{
    /// <summary>Allow CRL / OCSP servers on loopback, private and link-local addresses (an internal CA). Default false.</summary>
    public bool AllowPrivateNetworks { get; set; }

    /// <summary>Largest answer relayed, in bytes (default 10 MB).</summary>
    public long MaxBytes { get; set; } = 10 * 1024 * 1024;

    /// <summary>Upstream timeout (default 15 s).</summary>
    public TimeSpan Timeout { get; set; } = TimeSpan.FromSeconds(15);
}

/// <summary>Body of <c>POST /api/revocation/fetch</c>.</summary>
/// <param name="Kind"><c>crl</c> (HTTP GET, RFC 5280 §4.2.1.13) or <c>ocsp</c> (HTTP POST, RFC 6960 Appendix A.1).</param>
/// <param name="Url">The certificate's CRL distribution point or OCSP responder URL.</param>
/// <param name="Body">OCSP: the DER OCSPRequest, base64.</param>
/// <param name="Session">Editor session id (with the <c>X-Session-Token</c> header) when the caller is not signed in.</param>
public sealed record RevocationFetchRequest(string Kind, string Url, string? Body, string? Session);

/// <summary>
/// Fetches CRLs and OCSP responses for the browser, which cannot reach certificate authorities itself (CORS), so that
/// the WASM verifier can check certificate revocation (ADR-0007). Only http/https URLs; by default no loopback, private
/// or link-local address — checked when each connection is made, so DNS rebinding and redirects cannot reach the
/// internal network; answers are size-limited.
/// </summary>
public static class RevocationProxy
{
    /// <summary>Named HttpClient.</summary>
    public const string HttpClientName = "revocation";

    /// <summary>Registers the options and the HttpClient.</summary>
    public static IServiceCollection AddRevocationProxy(this IServiceCollection services, IConfiguration config)
    {
        var opts = new RevocationOptions();
        config.GetSection("Revocation").Bind(opts);
        services.AddSingleton(opts);
        services.AddHttpClient(HttpClientName, c => c.Timeout = opts.Timeout)
            .ConfigurePrimaryHttpMessageHandler(() => new SocketsHttpHandler
            {
                AllowAutoRedirect = true,
                MaxAutomaticRedirections = 3,
                UseProxy = false,
                ConnectCallback = (ctx, ct) => ConnectAsync(ctx, opts, ct),
            });
        return services;
    }

    /// <summary>Is <paramref name="ip"/> a public address (not loopback, private, link-local, unique-local, multicast …)?</summary>
    public static bool IsPublic(IPAddress ip)
    {
        if (ip.IsIPv4MappedToIPv6)
        {
            ip = ip.MapToIPv4();
        }

        if (IPAddress.IsLoopback(ip) || ip.Equals(IPAddress.Any) || ip.Equals(IPAddress.IPv6Any) || ip.Equals(IPAddress.IPv6None))
        {
            return false;
        }

        if (ip.AddressFamily == AddressFamily.InterNetwork)
        {
            var b = ip.GetAddressBytes();
            return !(b[0] == 10 // 10/8
                || (b[0] == 172 && b[1] >= 16 && b[1] <= 31) // 172.16/12
                || (b[0] == 192 && b[1] == 168) // 192.168/16
                || (b[0] == 169 && b[1] == 254) // link-local
                || (b[0] == 100 && b[1] >= 64 && b[1] <= 127) // CGNAT 100.64/10
                || b[0] == 0 || b[0] >= 224); // "this" network, multicast, reserved
        }

        return !(ip.IsIPv6LinkLocal || ip.IsIPv6SiteLocal || ip.IsIPv6Multicast
            || (ip.GetAddressBytes()[0] & 0xFE) == 0xFC); // fc00::/7 unique local
    }

    private static async ValueTask<Stream> ConnectAsync(SocketsHttpConnectionContext ctx, RevocationOptions opts, CancellationToken ct)
    {
        var host = ctx.DnsEndPoint.Host;
        var addresses = IPAddress.TryParse(host, out var literal) ? [literal] : await Dns.GetHostAddressesAsync(host, ct);
        var allowed = addresses.Where(a => opts.AllowPrivateNetworks || IsPublic(a)).ToArray();
        if (allowed.Length == 0)
        {
            throw new HttpRequestException($"{host}: address not allowed");
        }

        var socket = new Socket(SocketType.Stream, ProtocolType.Tcp) { NoDelay = true };
        try
        {
            await socket.ConnectAsync(allowed, ctx.DnsEndPoint.Port, ct);
            return new NetworkStream(socket, ownsSocket: true);
        }
        catch
        {
            socket.Dispose();
            throw;
        }
    }

    /// <summary>Maps <c>POST /api/revocation/fetch</c>.</summary>
    public static void MapRevocationProxy(this IEndpointRouteBuilder app)
    {
        app.MapPost("/api/revocation/fetch", FetchAsync).AllowAnonymous();
    }

    private static async Task<IResult> FetchAsync(
        RevocationFetchRequest body,
        HttpRequest req,
        ClaimsPrincipal user,
        SessionStore sessions,
        TimeProvider time,
        IHttpClientFactory http,
        RevocationOptions opts,
        CancellationToken ct)
    {
        var signedIn = user.Identity?.IsAuthenticated == true;
        var inSession = body.Session is { Length: > 0 }
            && sessions.Get(body.Session, req.Headers[SessionEndpoints.TokenHeader], time.GetUtcNow().UtcDateTime) is not null;
        if (!signedIn && !inSession)
        {
            return Results.Unauthorized();
        }

        if (!Uri.TryCreate(body.Url, UriKind.Absolute, out var url) || (url.Scheme != Uri.UriSchemeHttp && url.Scheme != Uri.UriSchemeHttps)
            || body.Url.Length > 2048 || !string.IsNullOrEmpty(url.UserInfo))
        {
            throw ApiException.BadRequest("url must be an http(s) URL.");
        }

        using var msg = body.Kind switch
        {
            "crl" => new HttpRequestMessage(HttpMethod.Get, url),
            "ocsp" => OcspRequest(url, body.Body),
            _ => throw ApiException.BadRequest("kind must be crl or ocsp."),
        };
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
                return Results.Problem("answer too large", statusCode: StatusCodes.Status502BadGateway);
            }

            await using var stream = await resp.Content.ReadAsStreamAsync(ct);
            using var buffer = new MemoryStream();
            var chunk = new byte[81920];
            int n;
            while ((n = await stream.ReadAsync(chunk, ct)) > 0)
            {
                if (buffer.Length + n > opts.MaxBytes)
                {
                    return Results.Problem("answer too large", statusCode: StatusCodes.Status502BadGateway);
                }

                buffer.Write(chunk, 0, n);
            }

            return Results.Bytes(buffer.ToArray(), "application/octet-stream");
        }
    }

    private static HttpRequestMessage OcspRequest(Uri url, string? body)
    {
        byte[] der;
        try
        {
            der = Convert.FromBase64String(body ?? "");
        }
        catch (FormatException)
        {
            throw ApiException.BadRequest("body must be base64.");
        }

        if (der.Length is 0 or > 64 * 1024)
        {
            throw ApiException.BadRequest("body must be an OCSP request.");
        }

        var content = new ByteArrayContent(der);
        content.Headers.ContentType = new("application/ocsp-request");
        return new HttpRequestMessage(HttpMethod.Post, url) { Content = content };
    }
}
