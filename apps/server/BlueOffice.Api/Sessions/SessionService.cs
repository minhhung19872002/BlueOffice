using System.Security.Claims;
using System.Security.Cryptography;
using BlueOffice.Api.Audit;
using BlueOffice.Api.Auth;
using BlueOffice.Api.Collab;
using BlueOffice.Api.Common;
using BlueOffice.Domain;
using BlueOffice.Infrastructure.Hosts;
using Microsoft.Extensions.Options;

namespace BlueOffice.Api.Sessions;

/// <summary>File part of <see cref="SessionInfo"/>.</summary>
public sealed record SessionFile(string Id, string Name, long Size, string Version, string? OwnerId, string? ShareUrl, DateTime? LastModified);

/// <summary>Collaboration endpoint of a session.</summary>
public sealed record SessionCollab(string Url, string Protocol);

/// <summary>Session-scoped REST endpoints.</summary>
public sealed record SessionEndpointsDto(string Self, string Contents, string Checkpoint, string Refresh, string Audit, string Versions);

/// <summary>What the editor needs to start (response of <c>GET /edit</c> and <c>POST /api/sessions</c>).</summary>
public sealed record SessionInfo(
    string SessionId,
    string? SessionToken,
    DateTime ExpiresAt,
    string Host,
    string Protocol,
    SessionFile File,
    HostUser User,
    HostPermissions Permissions,
    string Mode,
    SessionCollab Collab,
    SessionEndpointsDto Endpoints,
    IReadOnlyList<string> AllowedOrigins,
    int AutosaveSeconds);

/// <summary>POST /api/sessions body. Host session: Host+FileId+Token; WOPI: WopiSrc+Token; local: FileId only (with a user JWT).</summary>
public sealed record CreateSessionRequest(string? Host, string? FileId, string? Token, string? WopiSrc);

/// <summary>Creates, refreshes and revalidates editor sessions.</summary>
public sealed class SessionService(
    SessionStore store,
    HostRegistry hosts,
    HostTokenValidator validator,
    IBohpClient bohp,
    IWopiClient wopi,
    LocalHostStorage local,
    RoomManager rooms,
    AuditService audit,
    UserService users,
    IOptions<CollabOptions> options,
    TimeProvider time,
    ILogger<SessionService> logger)
{
    private DateTime Now => time.GetUtcNow().UtcDateTime;

    /// <summary>Session for a file of a configured host (BOHP token validated with the host JWKS; WOPI token checked by CheckFileInfo).</summary>
    public async Task<SessionInfo> CreateHostSessionAsync(string hostName, string fileId, string? token, CancellationToken ct)
    {
        var host = hosts.Find(hostName) ?? throw ApiException.NotFound("Host");
        if (string.IsNullOrEmpty(token))
        {
            throw new ApiException(StatusCodes.Status401Unauthorized, "Missing host access token.");
        }

        IHostStorage storage;
        switch (host.Protocol)
        {
            case HostProtocol.Bohp:
                var claims = await validator.ValidateAsync(host, token, ct);
                if (claims.FileId is { } fid && fid != fileId)
                {
                    throw ApiException.Forbidden("Token is not valid for this file.");
                }

                storage = bohp;
                break;
            case HostProtocol.Wopi:
                storage = wopi;
                break;
            default:
                throw ApiException.BadRequest("Unsupported host protocol.");
        }

        var info = await storage.GetFileAsync(new HostCallContext(host, token), fileId, ct);
        return await StartAsync(host, storage, fileId, token, info, ct);
    }

    /// <summary>Session from a WOPISrc URL (must belong to a configured WOPI host).</summary>
    public Task<SessionInfo> CreateWopiSessionAsync(string wopiSrc, string? token, CancellationToken ct)
    {
        var match = hosts.FindWopi(wopiSrc) ?? throw ApiException.Forbidden("WOPISrc does not belong to a configured host.");
        return CreateHostSessionAsync(match.Host.Name, match.FileId, token, ct);
    }

    /// <summary>Session for a BlueOffice-managed file (user authenticated with OIDC).</summary>
    public async Task<SessionInfo> CreateLocalSessionAsync(ClaimsPrincipal principal, string fileId, CancellationToken ct)
    {
        var user = await users.CurrentAsync(principal, ct);
        var token = user.Id.ToString();
        var info = await local.GetFileAsync(new HostCallContext(LocalHostStorage.Host, token), fileId, ct);
        return await StartAsync(LocalHostStorage.Host, local, fileId, token, info, ct);
    }

    /// <summary>Creates a session object for a WebSocket connection authenticated directly with a user JWT.</summary>
    public async Task<EditorSession> CreateLocalSessionObjectAsync(ClaimsPrincipal principal, string fileId, CancellationToken ct)
    {
        var info = await CreateLocalSessionAsync(principal, fileId, ct);
        return store.Get(info.SessionId, info.SessionToken, Now)!;
    }

    private async Task<SessionInfo> StartAsync(HostConfig host, IHostStorage storage, string fileId, string token, HostFileInfo info, CancellationToken ct)
    {
        if (!info.Permissions.View)
        {
            throw ApiException.Forbidden("No permission to open this file.");
        }

        var s = new EditorSession
        {
            Id = Guid.NewGuid().ToString("N"),
            Token = Convert.ToBase64String(RandomNumberGenerator.GetBytes(32)).TrimEnd('=').Replace('+', '-').Replace('/', '_'),
            Host = host,
            Storage = storage,
            FileId = fileId,
            AccessToken = token,
            File = info,
            CreatedAt = Now,
            ExpiresAt = Now + options.Value.SessionLifetime,
        };
        store.Add(s);
        await audit.RecordAsync(AuditAction.Open, ResourceKind.File, fileId, info.User.Id, info.User.Name, $"mode={info.Permissions.Mode}", host, ct);
        return Info(s, includeToken: true);
    }

    /// <summary>Session info for the editor.</summary>
    public SessionInfo Info(EditorSession s, bool includeToken)
    {
        var mode = rooms.Find(s.RoomKey)?.ModeOf(s) ?? s.Permissions.Mode;
        var f = s.File;
        var basePath = $"/api/sessions/{s.Id}";
        return new SessionInfo(
            s.Id,
            includeToken ? s.Token : null,
            s.ExpiresAt,
            s.Host.Name,
            s.Host.Protocol.ToString().ToLowerInvariant(),
            new SessionFile(s.FileId, f.Name, f.Size, f.Version, f.OwnerId, f.ShareUrl, f.LastModified),
            f.User,
            f.Permissions,
            mode,
            new SessionCollab($"/collab/{Uri.EscapeDataString(s.FileId)}?session={s.Id}", "y-protocols/v1+bo-json"),
            new SessionEndpointsDto(basePath, basePath + "/contents", basePath + "/checkpoint", basePath + "/refresh", basePath + "/audit", basePath + "/versions"),
            s.Host.AllowedOrigins,
            (int)options.Value.AutosaveInterval.TotalSeconds);
    }

    /// <summary>
    /// Re-reads permissions from the host (optionally with a new access token). A change is pushed to the session's
    /// connections as <c>mode-changed</c>; losing all access closes them.
    /// </summary>
    public async Task<SessionInfo> RefreshAsync(EditorSession s, string? newToken, CancellationToken ct)
    {
        if (!string.IsNullOrEmpty(newToken) && s.Host.Protocol != HostProtocol.Local)
        {
            if (s.Host.Protocol == HostProtocol.Bohp)
            {
                var claims = await validator.ValidateAsync(s.Host, newToken, ct);
                if (claims.Subject != s.User.Id)
                {
                    throw ApiException.Forbidden("Token belongs to another user.");
                }
            }

            s.AccessToken = newToken;
        }

        HostFileInfo info;
        try
        {
            info = await s.Storage.GetFileAsync(s.Context, s.FileId, ct);
        }
        catch (HostException e) when (e.StatusCode is 401 or 403 or 404)
        {
            logger.LogInformation("Access to {File} revoked for {User} ({Status})", s.FileId, s.User.Id, e.StatusCode);
            info = s.File with { Permissions = HostPermissions.None };
        }

        var changed = info.Permissions != s.Permissions;
        s.File = info with { User = s.File.User.Id.Length > 0 && info.User.Id.Length == 0 ? s.File.User : info.User };
        s.ExpiresAt = Now + options.Value.SessionLifetime;
        if (changed)
        {
            await rooms.PermissionsChangedAsync(s);
            if (!info.Permissions.View)
            {
                store.Remove(s.Id);
            }
        }

        return Info(s, includeToken: false);
    }

    /// <summary>Revalidates every session of a host file (optionally only one user's) — host notification.</summary>
    public async Task<int> RevalidateAsync(string hostName, string fileId, string? userId, CancellationToken ct)
    {
        var matches = store.All.Where(s => s.Host.Name.Equals(hostName, StringComparison.OrdinalIgnoreCase)
            && s.FileId == fileId && (userId is null || s.User.Id == userId)).ToList();
        foreach (var s in matches)
        {
            await RefreshAsync(s, null, ct);
        }

        return matches.Count;
    }
}
