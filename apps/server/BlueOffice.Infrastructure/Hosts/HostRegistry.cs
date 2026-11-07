using System.Collections.Concurrent;
using System.Security.Claims;
using BlueOffice.Domain;
using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Options;
using Microsoft.IdentityModel.JsonWebTokens;
using Microsoft.IdentityModel.Tokens;

namespace BlueOffice.Infrastructure.Hosts;

/// <summary>Configured hosts (bound from the <c>Hosts</c> array).</summary>
public sealed class HostsOptions
{
    /// <summary>Hosts.</summary>
    public List<HostConfig> Items { get; } = [];
}

/// <summary>Lookup of configured hosts.</summary>
public sealed class HostRegistry(IOptionsMonitor<HostsOptions> options)
{
    /// <summary>All hosts (entries without a name or base URL — e.g. unset deployment variables — are ignored).</summary>
    public IReadOnlyList<HostConfig> All =>
        options.CurrentValue.Items.Where(h => !string.IsNullOrWhiteSpace(h.Name) && !string.IsNullOrWhiteSpace(h.BaseUrl)).ToList();

    /// <summary>Host by name (case-insensitive).</summary>
    public HostConfig? Find(string? name) =>
        name is null ? null : All.FirstOrDefault(h => string.Equals(h.Name, name, StringComparison.OrdinalIgnoreCase));

    /// <summary>BOHP host whose <see cref="HostConfig.Issuer"/> equals <paramref name="issuer"/>.</summary>
    public HostConfig? FindByIssuer(string? issuer) =>
        issuer is null ? null : All.FirstOrDefault(h => h.Protocol == HostProtocol.Bohp && h.Issuer == issuer);

    /// <summary>
    /// WOPI host whose base URL is a prefix of <paramref name="wopiSrc"/>; returns the file id (last segment).
    /// Only configured hosts are reachable (prevents SSRF through arbitrary WOPISrc values).
    /// </summary>
    public (HostConfig Host, string FileId)? FindWopi(string wopiSrc)
    {
        if (!Uri.TryCreate(wopiSrc, UriKind.Absolute, out var src))
        {
            return null;
        }

        foreach (var h in All.Where(h => h.Protocol == HostProtocol.Wopi))
        {
            var prefix = h.BaseUrl.TrimEnd('/') + "/";
            var path = src.GetLeftPart(UriPartial.Path);
            if (path.StartsWith(prefix, StringComparison.OrdinalIgnoreCase))
            {
                var id = Uri.UnescapeDataString(path[prefix.Length..]);
                if (id.Length > 0 && !id.Contains('/', StringComparison.Ordinal))
                {
                    return (h, id);
                }
            }
        }

        return null;
    }
}

/// <summary>Claims of a validated host access token.</summary>
public sealed record HostTokenClaims(HostConfig Host, string Subject, string? Name, string? FileId, DateTime ExpiresAt, ClaimsIdentity Identity);

/// <summary>Invalid host token.</summary>
public sealed class HostTokenException(string message) : Exception(message);

/// <summary>Validates JWTs issued by a host against the host's JWKS (cached, refreshed on unknown <c>kid</c>).</summary>
public sealed class HostTokenValidator(IHttpClientFactory httpFactory, TimeProvider time, ILogger<HostTokenValidator> logger)
{
    /// <summary>Name of the HttpClient used to download JWKS documents.</summary>
    public const string HttpClientName = "jwks";

    private static readonly TimeSpan CacheTtl = TimeSpan.FromMinutes(10);
    private static readonly TimeSpan MinRefresh = TimeSpan.FromSeconds(30);
    private readonly ConcurrentDictionary<string, (JsonWebKeySet Keys, DateTime FetchedAt)> _cache = new();
    private readonly JsonWebTokenHandler _handler = new();

    /// <summary>Validates <paramref name="token"/> for <paramref name="host"/>; throws <see cref="HostTokenException"/>.</summary>
    public async Task<HostTokenClaims> ValidateAsync(HostConfig host, string token, CancellationToken ct)
    {
        if (string.IsNullOrEmpty(host.JwksUrl))
        {
            throw new HostTokenException($"Host '{host.Name}' has no JwksUrl.");
        }

        var keys = await GetKeysAsync(host, force: false, ct);
        var result = await _handler.ValidateTokenAsync(token, Parameters(host, keys));
        if (!result.IsValid && result.Exception is SecurityTokenSignatureKeyNotFoundException
            && time.GetUtcNow().UtcDateTime - keys.FetchedAt > MinRefresh)
        {
            // Key rotation: refetch the JWKS once.
            keys = await GetKeysAsync(host, force: true, ct);
            result = await _handler.ValidateTokenAsync(token, Parameters(host, keys));
        }

        if (!result.IsValid)
        {
            logger.LogInformation("Rejected token for host {Host}: {Reason}", host.Name, result.Exception?.Message);
            throw new HostTokenException("Invalid host token.");
        }

        var jwt = (JsonWebToken)result.SecurityToken;
        var id = result.ClaimsIdentity;
        return new HostTokenClaims(
            host,
            jwt.Subject ?? throw new HostTokenException("Token has no subject."),
            id.FindFirst("name")?.Value,
            id.FindFirst("fid")?.Value,
            jwt.ValidTo,
            id);
    }

    private TokenValidationParameters Parameters(HostConfig host, (JsonWebKeySet Keys, DateTime FetchedAt) keys) => new()
    {
        ValidIssuer = host.Issuer,
        ValidateIssuer = !string.IsNullOrEmpty(host.Issuer),
        ValidAudience = host.Audience,
        ValidateAudience = true,
        ValidateLifetime = true,
        RequireExpirationTime = true,
        ClockSkew = TimeSpan.FromMinutes(1),
        IssuerSigningKeys = keys.Keys.GetSigningKeys(),
        LifetimeValidator = (nbf, exp, _, _) =>
        {
            var now = time.GetUtcNow().UtcDateTime;
            return exp is { } e && e.AddMinutes(1) > now && (nbf is null || nbf.Value.AddMinutes(-1) <= now);
        },
    };

    private async Task<(JsonWebKeySet Keys, DateTime FetchedAt)> GetKeysAsync(HostConfig host, bool force, CancellationToken ct)
    {
        var now = time.GetUtcNow().UtcDateTime;
        if (!force && _cache.TryGetValue(host.Name, out var cached) && now - cached.FetchedAt < CacheTtl)
        {
            return cached;
        }

        var http = httpFactory.CreateClient(HttpClientName);
        string json;
        try
        {
            json = await http.GetStringAsync(new Uri(host.JwksUrl!), ct);
        }
        catch (HttpRequestException e)
        {
            throw new HostTokenException($"Cannot fetch JWKS of host '{host.Name}': {e.Message}");
        }

        var entry = (new JsonWebKeySet(json), now);
        _cache[host.Name] = entry;
        return entry;
    }
}
