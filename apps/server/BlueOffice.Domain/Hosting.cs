namespace BlueOffice.Domain;

/// <summary>Protocol spoken by a storage host.</summary>
public enum HostProtocol
{
    /// <summary>BlueOffice Host Protocol (docs/bohp.md), implemented by BlueDrive.</summary>
    Bohp,

    /// <summary>[MS-WOPI] (Nextcloud, SharePoint on-premises, …); BlueOffice acts as the WOPI client.</summary>
    Wopi,

    /// <summary>Files stored by BlueOffice itself (Phase 6 file API).</summary>
    Local,
}

/// <summary>Configuration of one storage host (section <c>Hosts</c>, one array item per host).</summary>
public sealed class HostConfig
{
    /// <summary>Host name used in URLs (<c>?host=bluedrive</c>).</summary>
    public string Name { get; set; } = "";

    /// <summary>Protocol.</summary>
    public HostProtocol Protocol { get; set; } = HostProtocol.Bohp;

    /// <summary>BOHP base URL (e.g. https://drive.example/bohp/v1) or WOPI files endpoint (…/wopi/files).</summary>
    public string BaseUrl { get; set; } = "";

    /// <summary>JWKS URL used to validate access tokens issued by the host (BOHP only).</summary>
    public string? JwksUrl { get; set; }

    /// <summary>Expected <c>iss</c> of host tokens.</summary>
    public string? Issuer { get; set; }

    /// <summary>Expected <c>aud</c> of host tokens.</summary>
    public string Audience { get; set; } = "blueoffice";

    /// <summary>Origins allowed to embed the editor / receive postMessage events.</summary>
    public List<string> AllowedOrigins { get; set; } = [];

    /// <summary>Webhook endpoint of the host (file saved, session started/ended, audit).</summary>
    public string? WebhookUrl { get; set; }

    /// <summary>HMAC-SHA256 secret used to sign webhooks.</summary>
    public string? WebhookSecret { get; set; }

    /// <summary>Lock duration requested from the host.</summary>
    public int LockDurationSeconds { get; set; } = 1800;

    /// <summary>Name shown in the desktop Start screen (defaults to <see cref="Name"/>).</summary>
    public string? DisplayName { get; set; }

    /// <summary>
    /// Sign-in of the desktop Start screen (OAuth 2.0 authorization code + PKCE, RFC 7636) at the host's / shared SSO identity
    /// provider. The resulting access token must be accepted by the host's BOHP endpoints and verifiable with <see cref="JwksUrl"/>.
    /// Null: the desktop only offers pasting a host token (development / mock).
    /// </summary>
    public HostSignInConfig? SignIn { get; set; }
}

/// <summary>OAuth 2.0 / OIDC public-client settings used by the desktop to sign in to a host (section <c>Hosts:N:SignIn</c>). Not secret.</summary>
public sealed class HostSignInConfig
{
    /// <summary>Authorization endpoint (RFC 6749 §3.1).</summary>
    public string AuthorizeUrl { get; set; } = "";

    /// <summary>Token endpoint (RFC 6749 §3.2).</summary>
    public string TokenUrl { get; set; } = "";

    /// <summary>Public client id registered for the desktop apps.</summary>
    public string ClientId { get; set; } = "";

    /// <summary>Requested scopes (space separated).</summary>
    public string Scope { get; set; } = "openid profile";
}

/// <summary>Host + the (user's) access token used for a call.</summary>
public sealed record HostCallContext(HostConfig Host, string AccessToken);

/// <summary>Permissions of the current user on a host file (BOHP §GET /files/{id}).</summary>
public sealed record HostPermissions(bool View, bool Comment, bool Edit, bool Share, bool Download)
{
    /// <summary>Nothing allowed.</summary>
    public static HostPermissions None { get; } = new(false, false, false, false, false);

    /// <summary>Maps a local role to host-style permissions.</summary>
    public static HostPermissions FromRole(ShareRole role) => new(
        role >= ShareRole.Viewer,
        role >= ShareRole.Commenter,
        role >= ShareRole.Editor,
        role >= ShareRole.Owner,
        role >= ShareRole.Viewer);

    /// <summary>Editor mode implied by the permissions: "edit", "comment", "view" or "none".</summary>
    public string Mode => Edit ? "edit" : Comment ? "comment" : View ? "view" : "none";
}

/// <summary>A user as described by a host.</summary>
public sealed record HostUser(string Id, string Name, string? AvatarUrl);

/// <summary>File metadata from a host.</summary>
public sealed record HostFileInfo(
    string Id,
    string Name,
    long Size,
    string Version,
    string? OwnerId,
    HostPermissions Permissions,
    HostUser User,
    string? ShareUrl,
    DateTime? LastModified,
    string? LockId,
    bool SupportsLocks,
    bool SupportsVersions)
{
    /// <summary>Folder containing the file (BOHP <c>folderId</c>); target of Save As and conflict copies. Null if unknown.</summary>
    public string? FolderId { get; init; }
}

/// <summary>File content + the version it corresponds to.</summary>
public sealed record HostContent(byte[] Content, string? Version);

/// <summary>Options of a content write.</summary>
public sealed record HostPutOptions(string? IfMatchVersion, string? LockId, string? Note, IReadOnlyList<string> Editors);

/// <summary>Result of a content write.</summary>
public sealed record HostPutResult(string Version);

/// <summary>Result of a lock operation.</summary>
public sealed record HostLockResult(string LockId, DateTime? ExpiresAt);

/// <summary>A version in the host's history.</summary>
public sealed record HostVersion(string Version, long Size, DateTime? ModifiedAt, HostUser? ModifiedBy, string? Note);

/// <summary>A file created by Save As.</summary>
public sealed record HostCreatedFile(string Id, string Name, string Version);

/// <summary>Storage operations BlueOffice needs from a host. Implemented by the BOHP client, the WOPI client and local storage.</summary>
public interface IHostStorage
{
    /// <summary>Protocol implemented.</summary>
    HostProtocol Protocol { get; }

    /// <summary>Metadata + permissions of the calling user.</summary>
    Task<HostFileInfo> GetFileAsync(HostCallContext ctx, string fileId, CancellationToken ct);

    /// <summary>Current content.</summary>
    Task<HostContent> GetContentsAsync(HostCallContext ctx, string fileId, CancellationToken ct);

    /// <summary>Writes new content; throws <see cref="HostConflictException"/> if <see cref="HostPutOptions.IfMatchVersion"/> or the lock does not match.</summary>
    Task<HostPutResult> PutContentsAsync(HostCallContext ctx, string fileId, byte[] content, HostPutOptions options, CancellationToken ct);

    /// <summary>Takes (or re-takes with the same id) a lock.</summary>
    Task<HostLockResult> LockAsync(HostCallContext ctx, string fileId, string lockId, TimeSpan duration, CancellationToken ct);

    /// <summary>Releases a lock.</summary>
    Task UnlockAsync(HostCallContext ctx, string fileId, string lockId, CancellationToken ct);

    /// <summary>Extends a lock.</summary>
    Task<HostLockResult> RefreshLockAsync(HostCallContext ctx, string fileId, string lockId, TimeSpan duration, CancellationToken ct);

    /// <summary>Version history, newest first.</summary>
    Task<IReadOnlyList<HostVersion>> GetVersionsAsync(HostCallContext ctx, string fileId, CancellationToken ct);

    /// <summary>Content of one version of the history (open read-only, compare); 404 if unknown.</summary>
    Task<HostContent> GetVersionContentsAsync(HostCallContext ctx, string fileId, string version, CancellationToken ct);

    /// <summary>Restores an old version as the new current version.</summary>
    Task<HostPutResult> RestoreVersionAsync(HostCallContext ctx, string fileId, string version, CancellationToken ct);

    /// <summary>Renames; returns the final name.</summary>
    Task<string> RenameAsync(HostCallContext ctx, string fileId, string newName, CancellationToken ct);

    /// <summary>Creates a new file in a folder (Save As / new document).</summary>
    Task<HostCreatedFile> CreateFileAsync(HostCallContext ctx, string folderId, string name, byte[] content, CancellationToken ct);
}

/// <summary>Kind of an entry in a host folder listing.</summary>
public enum HostItemKind
{
    /// <summary>A folder.</summary>
    Folder,

    /// <summary>A file.</summary>
    File,
}

/// <summary>A file or folder in a host listing (BOHP §A2).</summary>
public sealed record HostItem(
    HostItemKind Kind,
    string Id,
    string Name,
    long? Size,
    string? Version,
    DateTime? LastModified,
    string? OwnerId,
    DateTime? OpenedAt);

/// <summary>A folder as described by a host.</summary>
public sealed record HostFolder(string Id, string Name, string? ParentId);

/// <summary>Children of a host folder.</summary>
public sealed record HostFolderListing(HostFolder Folder, IReadOnlyList<HostItem> Items);

/// <summary>A file found by its path inside the user's sync folder.</summary>
public sealed record HostResolvedFile(string Id, string Name, string? Version);

/// <summary>
/// Optional browsing operations (BOHP §A2) used by the desktop Start screen and the sync-folder detection. Called with the
/// user's host token (not scoped to one file).
/// </summary>
public interface IHostBrowser
{
    /// <summary>The signed-in user.</summary>
    Task<HostUser> GetMeAsync(HostCallContext ctx, CancellationToken ct);

    /// <summary>Folders and files of a folder (<c>root</c> = the user's root folder).</summary>
    Task<HostFolderListing> ListChildrenAsync(HostCallContext ctx, string folderId, CancellationToken ct);

    /// <summary>Files shared with the user.</summary>
    Task<IReadOnlyList<HostItem>> ListSharedAsync(HostCallContext ctx, CancellationToken ct);

    /// <summary>Files the user opened recently, newest first.</summary>
    Task<IReadOnlyList<HostItem>> ListRecentAsync(HostCallContext ctx, int limit, CancellationToken ct);

    /// <summary>File at <paramref name="relativePath"/> ('/'-separated) inside the user's sync folder; null if none.</summary>
    Task<HostResolvedFile?> ResolvePathAsync(HostCallContext ctx, string relativePath, CancellationToken ct);
}

/// <summary>Generic host failure carrying the HTTP status returned by the host.</summary>
public class HostException(int statusCode, string message) : Exception(message)
{
    /// <summary>HTTP status (0 = network failure).</summary>
    public int StatusCode { get; } = statusCode;
}

/// <summary>409: version (If-Match) or lock mismatch.</summary>
public sealed class HostConflictException(string reason, string? currentVersion, string? currentLockId)
    : HostException(409, $"Conflict ({reason}).")
{
    /// <summary>"version" or "lock".</summary>
    public string Reason { get; } = reason;

    /// <summary>Current version on the host, when known.</summary>
    public string? CurrentVersion { get; } = currentVersion;

    /// <summary>Current lock id on the host, when known.</summary>
    public string? CurrentLockId { get; } = currentLockId;
}

/// <summary>The operation is not offered by this protocol.</summary>
public sealed class HostNotSupportedException(string operation)
    : HostException(501, $"Operation '{operation}' is not supported by this host.");
