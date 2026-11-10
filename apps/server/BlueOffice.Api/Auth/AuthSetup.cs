using System.Security.Claims;
using System.Text;
using System.Text.Encodings.Web;
using BlueOffice.Infrastructure.Hosts;
using Microsoft.AspNetCore.Authentication;
using Microsoft.AspNetCore.Authentication.JwtBearer;
using Microsoft.Extensions.Options;
using Microsoft.IdentityModel.JsonWebTokens;
using Microsoft.IdentityModel.Tokens;

namespace BlueOffice.Api.Auth;

/// <summary>Options of section <c>Auth</c>.</summary>
public sealed class AuthOptions
{
    /// <summary>OIDC authority (Keycloak realm URL, e.g. https://sso.example/realms/blueoffice).</summary>
    public string? Authority { get; set; }

    /// <summary>Expected audience of user tokens.</summary>
    public string Audience { get; set; } = "blueoffice-api";

    /// <summary>Require HTTPS for the OIDC metadata endpoint.</summary>
    public bool RequireHttpsMetadata { get; set; } = true;

    /// <summary>DEV/TEST ONLY: HMAC key (≥ 32 bytes) used instead of OIDC. Refused in the Production environment.</summary>
    public string? DevSigningKey { get; set; }

    /// <summary>Issuer of dev tokens.</summary>
    public string DevIssuer { get; set; } = "blueoffice-dev";

    /// <summary>Public OIDC client of the web app (section <c>Auth:WebClient</c>); none = the web app stays anonymous.</summary>
    public WebClientOptions WebClient { get; set; } = new();
}

/// <summary>
/// Public (no secret) OIDC client used by the web app for the authorization code
/// flow with PKCE (RFC 6749 §4.1, RFC 7636), published by <c>GET /api/auth/config</c>.
/// </summary>
public sealed class WebClientOptions
{
    /// <summary>Client id registered at the provider; empty disables web sign-in.</summary>
    public string? ClientId { get; set; }

    /// <summary>Provider issuer for discovery (default: <c>Auth:Authority</c>).</summary>
    public string? Authority { get; set; }

    /// <summary>Explicit endpoints when the provider has no discovery document.</summary>
    public string? AuthorizeUrl { get; set; }

    /// <summary>Token endpoint (see <see cref="AuthorizeUrl"/>).</summary>
    public string? TokenUrl { get; set; }

    /// <summary>RP-initiated logout endpoint (optional).</summary>
    public string? EndSessionUrl { get; set; }

    /// <summary>Requested scopes.</summary>
    public string Scope { get; set; } = "openid profile";
}

/// <summary>Authentication schemes and policies.</summary>
public static class AuthSetup
{
    /// <summary>Scheme validating JWTs issued by a configured host (BOHP), see <see cref="HostJwtHandler"/>.</summary>
    public const string HostScheme = "HostJwt";

    /// <summary>Policy: BlueOffice user (OIDC).</summary>
    public const string UserPolicy = "User";

    /// <summary>Policy: BlueOffice user or a host service (BlueDrive) — used by the conversion endpoints.</summary>
    public const string ServicePolicy = "Service";

    /// <summary>Policy: host-signed token only.</summary>
    public const string HostPolicy = "Host";

    /// <summary>Claim carrying the host name on host-authenticated principals.</summary>
    public const string HostClaim = "bo_host";

    /// <summary>Registers JWT bearer (OIDC or dev key), the host scheme and policies.</summary>
    public static IServiceCollection AddBlueOfficeAuth(this IServiceCollection services, IConfiguration config, IHostEnvironment env)
    {
        var opts = new AuthOptions();
        config.GetSection("Auth").Bind(opts);
        if (!string.IsNullOrEmpty(opts.DevSigningKey) && env.IsProduction())
        {
            throw new InvalidOperationException("Auth:DevSigningKey must not be set in Production.");
        }

        services.AddAuthentication(JwtBearerDefaults.AuthenticationScheme)
            .AddJwtBearer(o =>
            {
                o.MapInboundClaims = false;
                o.TokenValidationParameters = new TokenValidationParameters
                {
                    NameClaimType = "name",
                    ValidAudience = opts.Audience,
                    ClockSkew = TimeSpan.FromMinutes(1),
                };
                if (!string.IsNullOrEmpty(opts.DevSigningKey))
                {
                    o.TokenValidationParameters.ValidIssuer = opts.DevIssuer;
                    o.TokenValidationParameters.IssuerSigningKey = new SymmetricSecurityKey(Encoding.UTF8.GetBytes(opts.DevSigningKey));
                }
                else
                {
                    o.Authority = opts.Authority;
                    o.RequireHttpsMetadata = opts.RequireHttpsMetadata;
                }

                o.Events = new JwtBearerEvents
                {
                    // Browsers cannot set headers on WebSocket requests: accept ?access_token= on /collab only.
                    OnMessageReceived = ctx =>
                    {
                        if (ctx.HttpContext.Request.Path.StartsWithSegments("/collab")
                            && ctx.Request.Query.TryGetValue("access_token", out var t))
                        {
                            ctx.Token = t;
                        }

                        return Task.CompletedTask;
                    },
                };
            })
            .AddScheme<AuthenticationSchemeOptions, HostJwtHandler>(HostScheme, null);

        services.AddAuthorizationBuilder()
            .AddPolicy(UserPolicy, p => p.AddAuthenticationSchemes(JwtBearerDefaults.AuthenticationScheme).RequireAuthenticatedUser())
            .AddPolicy(ServicePolicy, p => p.AddAuthenticationSchemes(JwtBearerDefaults.AuthenticationScheme, HostScheme).RequireAuthenticatedUser())
            .AddPolicy(HostPolicy, p => p.AddAuthenticationSchemes(HostScheme).RequireClaim(HostClaim));
        return services;
    }

    /// <summary>
    /// <c>GET /api/auth/config</c> (anonymous): the web app's OIDC client settings,
    /// <c>{ "oidc": null }</c> when web sign-in is not configured.
    /// </summary>
    public static void MapAuthConfig(this IEndpointRouteBuilder app, IConfiguration config)
    {
        var opts = new AuthOptions();
        config.GetSection("Auth").Bind(opts);
        var w = opts.WebClient;
        object? oidc = string.IsNullOrWhiteSpace(w.ClientId)
            ? null
            : new
            {
                authority = string.IsNullOrWhiteSpace(w.Authority) ? opts.Authority : w.Authority,
                authorizeUrl = w.AuthorizeUrl,
                tokenUrl = w.TokenUrl,
                endSessionUrl = w.EndSessionUrl,
                clientId = w.ClientId,
                scope = w.Scope,
            };
        app.MapGet("/api/auth/config", () => Results.Ok(new { oidc })).AllowAnonymous();
    }

    /// <summary>Claim carrying the key name on principals authenticated by a REST API key (<c>Rest/ApiKeyAuth.cs</c>).</summary>
    public const string ApiKeyClaim = "bo_api_key";

    /// <summary>Stable principal id: "host:&lt;name&gt;:&lt;sub&gt;", "apikey:&lt;name&gt;" or "user:&lt;sub&gt;".</summary>
    public static string PrincipalKey(this ClaimsPrincipal p) =>
        p.FindFirst(HostClaim)?.Value is { } h ? $"host:{h}:{p.FindFirst("sub")?.Value}"
        : p.FindFirst(ApiKeyClaim)?.Value is { } k ? $"apikey:{k}"
        : $"user:{p.FindFirst("sub")?.Value}";
}

/// <summary>Authenticates <c>Authorization: Bearer</c> JWTs signed by a configured host (issuer → host → JWKS).</summary>
public sealed class HostJwtHandler(
    IOptionsMonitor<AuthenticationSchemeOptions> options,
    ILoggerFactory logger,
    UrlEncoder encoder,
    HostRegistry hosts,
    HostTokenValidator validator)
    : AuthenticationHandler<AuthenticationSchemeOptions>(options, logger, encoder)
{
    /// <inheritdoc />
    protected override async Task<AuthenticateResult> HandleAuthenticateAsync()
    {
        var header = Request.Headers.Authorization.ToString();
        if (!header.StartsWith("Bearer ", StringComparison.OrdinalIgnoreCase))
        {
            return AuthenticateResult.NoResult();
        }

        var token = header["Bearer ".Length..].Trim();
        string? issuer;
        try
        {
            issuer = new JsonWebToken(token).Issuer;
        }
        catch (ArgumentException)
        {
            return AuthenticateResult.NoResult();
        }

        var host = hosts.FindByIssuer(issuer);
        if (host is null)
        {
            return AuthenticateResult.NoResult();
        }

        try
        {
            var claims = await validator.ValidateAsync(host, token, Context.RequestAborted);
            var id = new ClaimsIdentity(claims.Identity.Claims, Scheme.Name, "name", "role");
            id.AddClaim(new Claim(AuthSetup.HostClaim, host.Name));
            return AuthenticateResult.Success(new AuthenticationTicket(new ClaimsPrincipal(id), Scheme.Name));
        }
        catch (HostTokenException e)
        {
            return AuthenticateResult.Fail(e.Message);
        }
    }
}
