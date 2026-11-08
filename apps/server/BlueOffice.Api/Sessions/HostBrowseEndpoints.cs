using BlueOffice.Api.Common;
using BlueOffice.Domain;
using BlueOffice.Infrastructure.Hosts;

namespace BlueOffice.Api.Sessions;

/// <summary>A configured host as listed for the desktop Start screen (no secrets).</summary>
public sealed record HostSummary(string Name, string DisplayName, string Protocol, bool Browse, HostSignInSummary? SignIn);

/// <summary>Public-client sign-in settings of a host (OAuth 2.0 authorization code + PKCE).</summary>
public sealed record HostSignInSummary(string AuthorizeUrl, string TokenUrl, string ClientId, string Scope);

/// <summary>
/// Proxy of the BOHP browsing endpoints (docs/bohp.md §A2) for the desktop Start screen and the sync-folder detection:
/// the desktop talks to one configured BlueOffice server (CORS / CSP already allow it) and never to the host directly.
/// The caller sends its host access token as <c>Authorization: Bearer</c>; it is validated against the host JWKS and then
/// forwarded unchanged to the host, which applies its own permissions.
/// </summary>
public static class HostBrowseEndpoints
{
    /// <summary>Maps the routes.</summary>
    public static void MapHostBrowseEndpoints(this IEndpointRouteBuilder app)
    {
        app.MapGet("/api/hosts", (HostRegistry hosts) => Results.Ok(new
        {
            hosts = hosts.All.Select(h => new HostSummary(
                h.Name,
                string.IsNullOrWhiteSpace(h.DisplayName) ? h.Name : h.DisplayName,
                h.Protocol.ToString().ToLowerInvariant(),
                h.Protocol == HostProtocol.Bohp,
                h.SignIn is { } si && !string.IsNullOrWhiteSpace(si.AuthorizeUrl) && !string.IsNullOrWhiteSpace(si.TokenUrl)
                    ? new HostSignInSummary(si.AuthorizeUrl, si.TokenUrl, si.ClientId, si.Scope)
                    : null)).ToList(),
        })).AllowAnonymous();

        var g = app.MapGroup("/api/hosts/{host}").AllowAnonymous();
        g.MapGet("/me", async (string host, HttpRequest req, HostRegistry hosts, HostTokenValidator validator, IBohpClient bohp, CancellationToken ct) =>
            Results.Ok(await bohp.GetMeAsync(await ContextAsync(host, req, hosts, validator, ct), ct)));
        g.MapGet("/folders/{folderId}/children", async (string host, string folderId, HttpRequest req, HostRegistry hosts, HostTokenValidator validator, IBohpClient bohp, CancellationToken ct) =>
            Results.Ok(await bohp.ListChildrenAsync(await ContextAsync(host, req, hosts, validator, ct), folderId, ct)));
        g.MapGet("/shared", async (string host, HttpRequest req, HostRegistry hosts, HostTokenValidator validator, IBohpClient bohp, CancellationToken ct) =>
            Results.Ok(new { items = await bohp.ListSharedAsync(await ContextAsync(host, req, hosts, validator, ct), ct) }));
        g.MapGet("/recent", async (string host, int? limit, HttpRequest req, HostRegistry hosts, HostTokenValidator validator, IBohpClient bohp, CancellationToken ct) =>
            Results.Ok(new { items = await bohp.ListRecentAsync(await ContextAsync(host, req, hosts, validator, ct), Math.Clamp(limit ?? 20, 1, 100), ct) }));
        g.MapGet("/resolve", async (string host, string? path, HttpRequest req, HostRegistry hosts, HostTokenValidator validator, IBohpClient bohp, CancellationToken ct) =>
        {
            var p = NormalizeRelativePath(path);
            var file = await bohp.ResolvePathAsync(await ContextAsync(host, req, hosts, validator, ct), p, ct);
            return file is null ? throw ApiException.NotFound("File") : Results.Ok(file);
        });
    }

    /// <summary>
    /// Checks a sync-folder relative path: '/'-separated (backslashes accepted), no empty, "." or ".." segments, at most 1024
    /// characters. Returns it with '/' separators.
    /// </summary>
    public static string NormalizeRelativePath(string? path)
    {
        var p = (path ?? "").Replace('\\', '/').Trim('/');
        var parts = p.Split('/');
        if (p.Length == 0 || p.Length > 1024 || parts.Any(s => s.Length == 0 || s == "." || s == ".." || s.Trim().Length == 0))
        {
            throw ApiException.BadRequest("Invalid relative path.");
        }

        return string.Join('/', parts);
    }

    private static async Task<HostCallContext> ContextAsync(string hostName, HttpRequest req, HostRegistry hosts, HostTokenValidator validator, CancellationToken ct)
    {
        var host = hosts.Find(hostName) ?? throw ApiException.NotFound("Host");
        if (host.Protocol != HostProtocol.Bohp)
        {
            throw new HostNotSupportedException("browse");
        }

        var header = req.Headers.Authorization.ToString();
        if (!header.StartsWith("Bearer ", StringComparison.OrdinalIgnoreCase))
        {
            throw new ApiException(StatusCodes.Status401Unauthorized, "Missing host access token.");
        }

        var token = header["Bearer ".Length..].Trim();

        // Throws HostTokenException (401) for tokens not signed by this host.
        await validator.ValidateAsync(host, token, ct);
        return new HostCallContext(host, token);
    }
}
