using System.Security.Claims;
using System.Text.Json;
using BlueOffice.Api.Auth;
using BlueOffice.Api.Sessions;
using BlueOffice.Domain;
using BlueOffice.Infrastructure.Data;
using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.Options;

namespace BlueOffice.Api.Addins;

/// <summary>Options of section <c>Addins</c> (docs/deploy/addins.md).</summary>
public sealed class AddinOptions
{
    /// <summary>Off by default: every add-in route answers 404 and the editors show no Add-in tab.</summary>
    public bool Enabled { get; set; }

    /// <summary>
    /// Origins add-ins may come from (their <c>allowedOrigins</c> and <c>network:</c> permissions). Empty: any https
    /// origin the administrator publishes.
    /// </summary>
    public List<string> ApprovedOrigins { get; set; } = [];

    /// <summary>Accept <c>http://localhost</c> / <c>http://127.0.0.1</c> origins (add-in development only).</summary>
    public bool AllowLoopbackOrigins { get; set; }

    /// <summary>Role that may publish add-ins (value of one of <see cref="RoleClaims"/>, or a Keycloak realm role).</summary>
    public string AdminRole { get; set; } = "blueoffice-admin";

    /// <summary>Claim types carrying roles.</summary>
    public List<string> RoleClaims { get; set; } = ["roles", "role", "groups"];

    /// <summary>Most add-ins in the catalogue.</summary>
    public int MaxAddins { get; set; } = 200;

    /// <summary>Largest manifest (JSON characters).</summary>
    public int MaxManifestChars { get; set; } = 64 * 1024;
}

/// <summary>Body of <c>PUT /api/admin/addins/{id}</c>.</summary>
public sealed record PublishAddinRequest(JsonElement? Manifest, bool? Trusted, bool? Enabled);

/// <summary>Body of <c>POST /api/addins/{id}/install</c>: the permissions the user consented to.</summary>
public sealed record InstallAddinRequest(string[]? Permissions);

/// <summary>
/// Organisation add-in catalogue (CORE-ADDIN-001, ADR-0013):
/// <list type="bullet">
/// <item>administrators (role <see cref="AddinOptions.AdminRole"/>): <c>GET /api/admin/addins</c>,
/// <c>PUT|DELETE /api/admin/addins/{id}</c>;</item>
/// <item>users — signed in (bearer) or through an editor session (<c>?session=</c> + <c>X-Session-Token</c>):
/// <c>GET /api/addins?app=doc</c>, <c>POST|DELETE /api/addins/{id}/install</c>.</item>
/// </list>
/// Every route answers 404 until <c>Addins:Enabled</c> is true.
/// </summary>
public static class AddinEndpoints
{
    private const string AuditCategory = "BlueOffice.Audit.Addins";

    /// <summary>Registers the options.</summary>
    public static IServiceCollection AddAddins(this IServiceCollection services, IConfiguration config)
    {
        services.Configure<AddinOptions>(config.GetSection("Addins"));
        return services;
    }

    /// <summary>Maps the routes.</summary>
    public static void MapAddins(this IEndpointRouteBuilder app)
    {
        var admin = app.MapGroup("/api/admin/addins").AllowAnonymous();
        admin.MapGet("", AdminListAsync);
        admin.MapPut("/{id}", PublishAsync);
        admin.MapDelete("/{id}", UnpublishAsync);

        var user = app.MapGroup("/api/addins").AllowAnonymous();
        user.MapGet("", ListAsync);
        user.MapPost("/{id}/install", InstallAsync);
        user.MapDelete("/{id}/install", UninstallAsync);
    }

    private static IResult Problem(int status, string code, string detail) =>
        Results.Problem(detail, statusCode: status, extensions: new Dictionary<string, object?> { ["code"] = code });

    private static IResult Off() => Problem(StatusCodes.Status404NotFound, "unavailable", "Add-ins are not enabled.");

    /// <summary>Whether <paramref name="p"/> carries the administrator role (role claims or Keycloak <c>realm_access.roles</c>).</summary>
    public static bool IsAdmin(ClaimsPrincipal p, AddinOptions o)
    {
        if (p.Identity?.IsAuthenticated != true || p.FindFirst(AuthSetup.HostClaim) is not null || string.IsNullOrEmpty(o.AdminRole))
        {
            return false;
        }

        if (p.Claims.Any(c => o.RoleClaims.Contains(c.Type) && c.Value == o.AdminRole))
        {
            return true;
        }

        foreach (var c in p.FindAll("realm_access"))
        {
            try
            {
                using var doc = JsonDocument.Parse(c.Value);
                if (doc.RootElement.ValueKind == JsonValueKind.Object
                    && doc.RootElement.TryGetProperty("roles", out var roles)
                    && roles.ValueKind == JsonValueKind.Array
                    && roles.EnumerateArray().Any(r => r.ValueKind == JsonValueKind.String && r.GetString() == o.AdminRole))
                {
                    return true;
                }
            }
            catch (JsonException)
            {
                // Not JSON: not a role list.
            }
        }

        return false;
    }

    /// <summary>The user calling: a signed-in principal or the user of an editor session.</summary>
    private static string? Caller(HttpRequest req, ClaimsPrincipal user, SessionStore sessions, TimeProvider time, string? session)
    {
        if (session is { Length: > 0 })
        {
            return sessions.Get(session, req.Headers[SessionEndpoints.TokenHeader], time.GetUtcNow().UtcDateTime) is { } s
                ? $"host:{s.Host.Name}:{s.User.Id}"
                : null;
        }

        return user.Identity?.IsAuthenticated == true && user.FindFirst(AuthSetup.ApiKeyClaim) is null ? user.PrincipalKey() : null;
    }

    private static object AdminDto(AddinEntry a, int installs) => new
    {
        id = a.Id,
        version = a.Version,
        name = a.Name,
        apps = a.Apps.Split(' ', StringSplitOptions.RemoveEmptyEntries),
        trusted = a.Trusted,
        enabled = a.Enabled,
        publishedBy = a.PublishedBy,
        createdAt = a.CreatedAt,
        updatedAt = a.UpdatedAt,
        installs,
        manifest = JsonDocument.Parse(a.Manifest).RootElement,
    };

    private static async Task<IResult> AdminListAsync(ClaimsPrincipal user, IOptions<AddinOptions> options, BlueOfficeDbContext db, CancellationToken ct)
    {
        var o = options.Value;
        if (!o.Enabled)
        {
            return Off();
        }

        if (!IsAdmin(user, o))
        {
            return user.Identity?.IsAuthenticated == true ? Results.Forbid() : Results.Unauthorized();
        }

        var list = await db.Addins.OrderBy(x => x.Id).ToListAsync(ct);
        var counts = await db.AddinInstalls.GroupBy(x => x.AddinId).Select(g => new { g.Key, Count = g.Count() }).ToDictionaryAsync(x => x.Key, x => x.Count, ct);
        return Results.Ok(new { addins = list.Select(a => AdminDto(a, counts.GetValueOrDefault(a.Id))) });
    }

    private static async Task<IResult> PublishAsync(
        string id,
        PublishAddinRequest body,
        ClaimsPrincipal user,
        IOptions<AddinOptions> options,
        BlueOfficeDbContext db,
        TimeProvider time,
        ILoggerFactory logs,
        CancellationToken ct)
    {
        var o = options.Value;
        if (!o.Enabled)
        {
            return Off();
        }

        if (!IsAdmin(user, o))
        {
            return user.Identity?.IsAuthenticated == true ? Results.Forbid() : Results.Unauthorized();
        }

        if (body.Manifest is not { } manifest)
        {
            return Problem(StatusCodes.Status400BadRequest, "invalid", "'manifest' is required.");
        }

        var json = manifest.GetRawText();
        if (json.Length > o.MaxManifestChars)
        {
            return Problem(StatusCodes.Status413PayloadTooLarge, "tooLarge", $"A manifest has at most {o.MaxManifestChars} characters.");
        }

        var m = AddinManifestValidator.Validate(manifest, o.AllowLoopbackOrigins, out var errors);
        if (m is not null && m.Id != id)
        {
            errors.Add(("id", "must equal the id in the URL"));
        }

        if (m is not null && o.ApprovedOrigins.Count > 0)
        {
            var network = m.Permissions.Where(p => p.StartsWith(AddinManifestValidator.NetworkPrefix, StringComparison.Ordinal)).Select(p => p[AddinManifestValidator.NetworkPrefix.Length..]);
            foreach (var origin in m.Origins.Concat(network).Where(x => !o.ApprovedOrigins.Contains(x)).Distinct())
            {
                errors.Add(("allowedOrigins", $"{origin} is not an approved add-in origin (Addins:ApprovedOrigins)"));
            }
        }

        if (errors.Count > 0)
        {
            return Results.Json(
                new { title = "The manifest is not valid.", status = 400, code = "invalidManifest", errors = errors.Select(e => new { path = e.Path, message = e.Message }) },
                statusCode: StatusCodes.Status400BadRequest);
        }

        var now = time.GetUtcNow().UtcDateTime;
        var entry = await db.Addins.FirstOrDefaultAsync(x => x.Id == id, ct);
        var created = entry is null;
        if (entry is null)
        {
            if (await db.Addins.CountAsync(ct) >= o.MaxAddins)
            {
                return Problem(StatusCodes.Status409Conflict, "full", $"The catalogue holds at most {o.MaxAddins} add-ins.");
            }

            entry = new AddinEntry { Id = id, CreatedAt = now };
            db.Addins.Add(entry);
        }

        entry!.Version = m!.Version;
        entry.Name = m.NameVi;
        entry.Apps = string.Join(' ', m.Apps);
        entry.Manifest = json;
        entry.Trusted = body.Trusted ?? (!created && entry.Trusted);
        entry.Enabled = body.Enabled ?? (created || entry.Enabled);
        entry.PublishedBy = user.PrincipalKey();
        entry.UpdatedAt = now;
        await db.SaveChangesAsync(ct);
        logs.CreateLogger(AuditCategory).LogInformation(
            "Add-in published: id={Id} version={Version} trusted={Trusted} by={By}", entry.Id, entry.Version, entry.Trusted, entry.PublishedBy);
        var dto = AdminDto(entry, await db.AddinInstalls.CountAsync(x => x.AddinId == id, ct));
        return created ? Results.Created($"/api/admin/addins/{id}", dto) : Results.Ok(dto);
    }

    private static async Task<IResult> UnpublishAsync(string id, ClaimsPrincipal user, IOptions<AddinOptions> options, BlueOfficeDbContext db, ILoggerFactory logs, CancellationToken ct)
    {
        var o = options.Value;
        if (!o.Enabled)
        {
            return Off();
        }

        if (!IsAdmin(user, o))
        {
            return user.Identity?.IsAuthenticated == true ? Results.Forbid() : Results.Unauthorized();
        }

        var entry = await db.Addins.FirstOrDefaultAsync(x => x.Id == id, ct);
        if (entry is null)
        {
            return Problem(StatusCodes.Status404NotFound, "notFound", "Add-in not found.");
        }

        db.AddinInstalls.RemoveRange(db.AddinInstalls.Where(x => x.AddinId == id));
        db.Addins.Remove(entry);
        await db.SaveChangesAsync(ct);
        logs.CreateLogger(AuditCategory).LogInformation("Add-in removed: id={Id} by={By}", id, user.PrincipalKey());
        return Results.NoContent();
    }

    private static async Task<IResult> ListAsync(
        HttpRequest req,
        ClaimsPrincipal user,
        SessionStore sessions,
        TimeProvider time,
        IOptions<AddinOptions> options,
        BlueOfficeDbContext db,
        string? app,
        string? session,
        CancellationToken ct)
    {
        if (!options.Value.Enabled)
        {
            return Off();
        }

        var caller = Caller(req, user, sessions, time, session);
        if (caller is null)
        {
            return Results.Unauthorized();
        }

        if (app is not null && !AddinManifestValidator.Apps.Contains(app))
        {
            return Problem(StatusCodes.Status400BadRequest, "invalid", "'app' must be doc, sheet or slide.");
        }

        var list = await db.Addins.Where(x => x.Enabled).OrderBy(x => x.Id).ToListAsync(ct);
        if (app is not null)
        {
            list = [.. list.Where(x => x.Apps.Split(' ').Contains(app))];
        }

        var mine = await db.AddinInstalls.Where(x => x.UserKey == caller).ToDictionaryAsync(x => x.AddinId, ct);
        return Results.Ok(new
        {
            addins = list.Select(a => new
            {
                manifest = JsonDocument.Parse(a.Manifest).RootElement,
                trusted = a.Trusted,
                installed = mine.ContainsKey(a.Id),
                granted = mine.TryGetValue(a.Id, out var i) ? i.Permissions.Split(' ', StringSplitOptions.RemoveEmptyEntries) : [],
            }),
        });
    }

    private static async Task<IResult> InstallAsync(
        string id,
        InstallAddinRequest body,
        HttpRequest req,
        ClaimsPrincipal user,
        SessionStore sessions,
        TimeProvider time,
        IOptions<AddinOptions> options,
        BlueOfficeDbContext db,
        ILoggerFactory logs,
        string? session,
        CancellationToken ct)
    {
        if (!options.Value.Enabled)
        {
            return Off();
        }

        var caller = Caller(req, user, sessions, time, session);
        if (caller is null)
        {
            return Results.Unauthorized();
        }

        var entry = await db.Addins.FirstOrDefaultAsync(x => x.Id == id && x.Enabled, ct);
        if (entry is null)
        {
            return Problem(StatusCodes.Status404NotFound, "notFound", "Add-in not found.");
        }

        using var doc = JsonDocument.Parse(entry.Manifest);
        var required = doc.RootElement.GetProperty("permissions").EnumerateArray().Select(p => p.GetString() ?? "").ToList();
        var granted = (body.Permissions ?? []).Distinct().ToList();
        // Consent is all-or-nothing: the user agreed to exactly what the manifest asks for.
        if (required.Count != granted.Count || required.Except(granted).Any())
        {
            return Problem(StatusCodes.Status400BadRequest, "consent", "'permissions' must list every permission of the manifest.");
        }

        var install = await db.AddinInstalls.FirstOrDefaultAsync(x => x.AddinId == id && x.UserKey == caller, ct);
        if (install is null)
        {
            install = new AddinInstall { AddinId = id, UserKey = caller };
            db.AddinInstalls.Add(install);
        }

        install.Permissions = string.Join(' ', required);
        install.Version = entry.Version;
        install.InstalledAt = time.GetUtcNow().UtcDateTime;
        await db.SaveChangesAsync(ct);
        logs.CreateLogger(AuditCategory).LogInformation("Add-in installed: id={Id} version={Version} user={User}", id, entry.Version, caller);
        return Results.Ok(new { installed = true, granted = required });
    }

    private static async Task<IResult> UninstallAsync(
        string id,
        HttpRequest req,
        ClaimsPrincipal user,
        SessionStore sessions,
        TimeProvider time,
        IOptions<AddinOptions> options,
        BlueOfficeDbContext db,
        ILoggerFactory logs,
        string? session,
        CancellationToken ct)
    {
        if (!options.Value.Enabled)
        {
            return Off();
        }

        var caller = Caller(req, user, sessions, time, session);
        if (caller is null)
        {
            return Results.Unauthorized();
        }

        var install = await db.AddinInstalls.FirstOrDefaultAsync(x => x.AddinId == id && x.UserKey == caller, ct);
        if (install is null)
        {
            return Problem(StatusCodes.Status404NotFound, "notFound", "Add-in is not installed.");
        }

        db.AddinInstalls.Remove(install);
        await db.SaveChangesAsync(ct);
        logs.CreateLogger(AuditCategory).LogInformation("Add-in uninstalled: id={Id} user={User}", id, caller);
        return Results.NoContent();
    }
}
