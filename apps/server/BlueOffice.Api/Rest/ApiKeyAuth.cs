using System.Security.Claims;
using System.Security.Cryptography;
using System.Text;
using System.Text.Encodings.Web;
using System.Threading.RateLimiting;
using BlueOffice.Api.Auth;
using BlueOffice.Api.Common;
using Microsoft.AspNetCore.Authentication;
using Microsoft.AspNetCore.Authentication.JwtBearer;
using Microsoft.AspNetCore.Mvc;
using Microsoft.AspNetCore.RateLimiting;
using Microsoft.Extensions.Options;

namespace BlueOffice.Api.Rest;

/// <summary>Options of the document REST API for other systems (section <c>RestApi</c>, docs/deploy/rest-api.md).</summary>
public sealed class RestApiOptions
{
    /// <summary>Serve <c>/api/v1/...</c> at all. Default false: the routes do not exist (404).</summary>
    public bool Enabled { get; set; }

    /// <summary>Also accept the bearer tokens of the <c>Service</c> policy (OIDC users, host services). False: API keys only.</summary>
    public bool AllowBearer { get; set; } = true;

    /// <summary>Largest uploaded file in bytes (never above <c>Storage:MaxUploadBytes</c>).</summary>
    public long MaxUploadBytes { get; set; } = 50L * 1024 * 1024;

    /// <summary>Largest number of edits of one <c>/documents/edit</c> request.</summary>
    public int MaxEdits { get; set; } = 1000;

    /// <summary>Largest number of records of a mail merge sent as JSON (<c>records</c>).</summary>
    public int MaxRecords { get; set; } = 5000;

    /// <summary>Requests per caller and time window.</summary>
    public RestRateLimitOptions RateLimit { get; set; } = new();

    /// <summary>API keys, managed by the administrator. Only the SHA-256 of a key is stored.</summary>
    public List<ApiKeyOptions> Keys { get; set; } = [];
}

/// <summary>Fixed-window rate limit of the REST API (per API key / user, per process).</summary>
public sealed class RestRateLimitOptions
{
    /// <summary>Requests allowed per <see cref="Window"/>; 0 = unlimited.</summary>
    public int PermitLimit { get; set; } = 60;

    /// <summary>Window length.</summary>
    public TimeSpan Window { get; set; } = TimeSpan.FromMinutes(1);
}

/// <summary>One API key of <see cref="RestApiOptions.Keys"/>.</summary>
public sealed class ApiKeyOptions
{
    /// <summary>Name of the client system (audit, job ownership, rate limit partition).</summary>
    public string Name { get; set; } = "";

    /// <summary>SHA-256 of the key, 64 hex digits (the key itself is never stored).</summary>
    public string Sha256 { get; set; } = "";

    /// <summary>Granted scopes (<see cref="ApiKeyAuth.AllScopes"/>); empty = all.</summary>
    public List<string> Scopes { get; set; } = [];

    /// <summary>False revokes the key without removing its entry.</summary>
    public bool Enabled { get; set; } = true;

    /// <summary>The key is refused from this time on (optional).</summary>
    public DateTimeOffset? ExpiresAt { get; set; }
}

/// <summary>API key authentication (<c>X-Api-Key</c>), scopes and rate limiting of the REST API.</summary>
public static class ApiKeyAuth
{
    /// <summary>Authentication scheme of API keys.</summary>
    public const string Scheme = "ApiKey";

    /// <summary>Request header carrying the key.</summary>
    public const string Header = "X-Api-Key";

    /// <summary>Policy of the REST API: an API key, or (unless disabled) a bearer token of the <c>Service</c> policy.</summary>
    public const string Policy = "RestApi";

    /// <summary>Rate limiter policy of the REST API.</summary>
    public const string RateLimitPolicy = "rest-api";

    /// <summary>Claim carrying one granted scope.</summary>
    public const string ScopeClaim = "bo_scope";

    /// <summary>Scopes of the REST API, one per operation.</summary>
    public static readonly IReadOnlyList<string> AllScopes = ["convert", "merge", "edit", "inspect"];

    /// <summary>Hex SHA-256 of a key, as stored in <see cref="ApiKeyOptions.Sha256"/>.</summary>
    public static string Hash(string key) => Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(key))).ToLowerInvariant();

    /// <summary>Registers the options (validated), the API key scheme, the policy and the rate limiter.</summary>
    public static IServiceCollection AddRestApi(this IServiceCollection services, IConfiguration config)
    {
        var opts = new RestApiOptions();
        config.GetSection("RestApi").Bind(opts);
        if (opts.Enabled)
        {
            Validate(opts);
        }

        services.AddSingleton(opts);
        services.AddAuthentication().AddScheme<AuthenticationSchemeOptions, ApiKeyHandler>(Scheme, null);
        services.AddAuthorizationBuilder().AddPolicy(Policy, p =>
        {
            p.AddAuthenticationSchemes(Scheme);
            if (opts.AllowBearer)
            {
                p.AddAuthenticationSchemes(JwtBearerDefaults.AuthenticationScheme, AuthSetup.HostScheme);
            }

            p.RequireAuthenticatedUser();
        });
        services.AddRateLimiter(o =>
        {
            o.RejectionStatusCode = StatusCodes.Status429TooManyRequests;
            o.OnRejected = async (ctx, ct) =>
            {
                if (ctx.Lease.TryGetMetadata(MetadataName.RetryAfter, out var retry))
                {
                    ctx.HttpContext.Response.Headers.RetryAfter = ((int)Math.Ceiling(retry.TotalSeconds)).ToString(System.Globalization.CultureInfo.InvariantCulture);
                }

                await ctx.HttpContext.RequestServices.GetRequiredService<IProblemDetailsService>().TryWriteAsync(new ProblemDetailsContext
                {
                    HttpContext = ctx.HttpContext,
                    ProblemDetails = new ProblemDetails { Status = StatusCodes.Status429TooManyRequests, Detail = "Rate limit exceeded." },
                });
            };

            // Runs after authorization: one window per API key / user (the caller is always authenticated here).
            o.AddPolicy(RateLimitPolicy, ctx => opts.RateLimit.PermitLimit <= 0
                ? RateLimitPartition.GetNoLimiter("")
                : RateLimitPartition.GetFixedWindowLimiter(ctx.User.PrincipalKey(), _ => new FixedWindowRateLimiterOptions
                {
                    PermitLimit = opts.RateLimit.PermitLimit,
                    Window = opts.RateLimit.Window,
                    QueueLimit = 0,
                }));
        });
        return services;
    }

    /// <summary>403 unless the caller may use <paramref name="scope"/> (bearer principals may use every operation).</summary>
    public static void RequireScope(this ClaimsPrincipal user, string scope)
    {
        if (user.FindFirst(AuthSetup.ApiKeyClaim) is not null && !user.HasClaim(ScopeClaim, scope))
        {
            throw ApiException.Forbidden($"The API key lacks the scope '{scope}'.");
        }
    }

    private static void Validate(RestApiOptions opts)
    {
        var names = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        foreach (var k in opts.Keys)
        {
            if (string.IsNullOrWhiteSpace(k.Name) || !names.Add(k.Name))
            {
                throw new InvalidOperationException("RestApi:Keys: every key needs a unique Name.");
            }

            if (k.Sha256.Length != 64 || !k.Sha256.All(Uri.IsHexDigit))
            {
                throw new InvalidOperationException($"RestApi:Keys '{k.Name}': Sha256 must be 64 hex digits (the SHA-256 of the key, not the key).");
            }

            if (k.Scopes.FirstOrDefault(s => !AllScopes.Contains(s)) is { } unknown)
            {
                throw new InvalidOperationException($"RestApi:Keys '{k.Name}': unknown scope '{unknown}'.");
            }
        }

        if (opts.MaxUploadBytes <= 0 || opts.MaxEdits <= 0 || opts.MaxRecords <= 0 || opts.RateLimit.Window <= TimeSpan.Zero)
        {
            throw new InvalidOperationException("RestApi: MaxUploadBytes, MaxEdits, MaxRecords and RateLimit:Window must be positive.");
        }
    }
}

/// <summary>Authenticates <c>X-Api-Key</c> against the configured key hashes (constant-time comparison).</summary>
public sealed class ApiKeyHandler(
    IOptionsMonitor<AuthenticationSchemeOptions> options,
    ILoggerFactory logger,
    UrlEncoder encoder,
    RestApiOptions api,
    TimeProvider time)
    : AuthenticationHandler<AuthenticationSchemeOptions>(options, logger, encoder)
{
    /// <inheritdoc />
    protected override Task<AuthenticateResult> HandleAuthenticateAsync()
    {
        if (!api.Enabled || !Request.Headers.TryGetValue(ApiKeyAuth.Header, out var values) || values.Count != 1 || string.IsNullOrEmpty(values[0]))
        {
            return Task.FromResult(AuthenticateResult.NoResult());
        }

        var presented = SHA256.HashData(Encoding.UTF8.GetBytes(values[0]!));
        ApiKeyOptions? match = null;
        foreach (var k in api.Keys)
        {
            // Every key is compared (no early exit): timing does not tell which entry matched.
            byte[] stored;
            try
            {
                stored = Convert.FromHexString(k.Sha256);
            }
            catch (FormatException)
            {
                continue;
            }

            if (CryptographicOperations.FixedTimeEquals(presented, stored))
            {
                match = k;
            }
        }

        if (match is null || !match.Enabled || (match.ExpiresAt is { } end && time.GetUtcNow() >= end))
        {
            return Task.FromResult(AuthenticateResult.Fail("Invalid API key."));
        }

        var id = new ClaimsIdentity(Scheme.Name, "name", "role");
        id.AddClaim(new Claim("sub", match.Name));
        id.AddClaim(new Claim("name", match.Name));
        id.AddClaim(new Claim(AuthSetup.ApiKeyClaim, match.Name));
        foreach (var s in match.Scopes.Count == 0 ? ApiKeyAuth.AllScopes : match.Scopes)
        {
            id.AddClaim(new Claim(ApiKeyAuth.ScopeClaim, s));
        }

        return Task.FromResult(AuthenticateResult.Success(new AuthenticationTicket(new ClaimsPrincipal(id), Scheme.Name)));
    }
}
