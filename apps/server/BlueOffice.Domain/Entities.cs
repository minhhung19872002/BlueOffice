namespace BlueOffice.Domain;

/// <summary>Sharing role. Ordered: a higher value includes every right of the lower ones.</summary>
public enum ShareRole
{
    /// <summary>No access.</summary>
    None = 0,

    /// <summary>Read and download.</summary>
    Viewer = 1,

    /// <summary>Viewer + comments.</summary>
    Commenter = 2,

    /// <summary>Commenter + edit content, rename, upload versions, restore.</summary>
    Editor = 3,

    /// <summary>Editor + delete, manage shares, read audit log.</summary>
    Owner = 4,
}

/// <summary>Kind of resource a share or audit entry points to.</summary>
public enum ResourceKind
{
    /// <summary>A file.</summary>
    File = 0,

    /// <summary>A folder (shares are inherited by everything inside).</summary>
    Folder = 1,
}

/// <summary>Audited actions (prompt §6A.6: who opened, edited, exported, printed, downloaded).</summary>
public enum AuditAction
{
    /// <summary>Document opened in the editor.</summary>
    Open,

    /// <summary>Document content changed (new version saved).</summary>
    Edit,

    /// <summary>Exported (PDF, other format).</summary>
    Export,

    /// <summary>Printed.</summary>
    Print,

    /// <summary>Raw content downloaded.</summary>
    Download,

    /// <summary>New file uploaded.</summary>
    Upload,

    /// <summary>Share or link created/removed.</summary>
    Share,

    /// <summary>Old version restored.</summary>
    Restore,

    /// <summary>File or folder renamed/moved.</summary>
    Rename,

    /// <summary>File or folder deleted.</summary>
    Delete,
}

/// <summary>A user known to BlueOffice (created on first authenticated request, keyed by the OIDC subject).</summary>
public sealed class User
{
    /// <summary>Internal id.</summary>
    public Guid Id { get; set; } = Guid.NewGuid();

    /// <summary>OIDC <c>sub</c> claim (unique).</summary>
    public string Subject { get; set; } = "";

    /// <summary>Display name.</summary>
    public string DisplayName { get; set; } = "";

    /// <summary>E-mail (optional).</summary>
    public string? Email { get; set; }

    /// <summary>Creation time (UTC).</summary>
    public DateTime CreatedAt { get; set; }
}

/// <summary>A folder. <see cref="ParentId"/> null = top level of the owner's drive.</summary>
public sealed class Folder
{
    /// <summary>Id.</summary>
    public Guid Id { get; set; } = Guid.NewGuid();

    /// <summary>Name.</summary>
    public string Name { get; set; } = "";

    /// <summary>Parent folder.</summary>
    public Guid? ParentId { get; set; }

    /// <summary>Owner.</summary>
    public Guid OwnerId { get; set; }

    /// <summary>Creation time (UTC).</summary>
    public DateTime CreatedAt { get; set; }

    /// <summary>Last change (UTC).</summary>
    public DateTime UpdatedAt { get; set; }

    /// <summary>Soft-deleted.</summary>
    public bool IsDeleted { get; set; }
}

/// <summary>A file; content lives in <see cref="FileVersion"/> objects.</summary>
public sealed class FileItem
{
    /// <summary>Id.</summary>
    public Guid Id { get; set; } = Guid.NewGuid();

    /// <summary>File name with extension.</summary>
    public string Name { get; set; } = "";

    /// <summary>Containing folder (null = top level).</summary>
    public Guid? FolderId { get; set; }

    /// <summary>Owner.</summary>
    public Guid OwnerId { get; set; }

    /// <summary>MIME type.</summary>
    public string ContentType { get; set; } = "application/octet-stream";

    /// <summary>Size of the current version.</summary>
    public long Size { get; set; }

    /// <summary>Current version number (1-based).</summary>
    public int CurrentVersion { get; set; }

    /// <summary>Creation time (UTC).</summary>
    public DateTime CreatedAt { get; set; }

    /// <summary>Last change (UTC).</summary>
    public DateTime UpdatedAt { get; set; }

    /// <summary>Soft-deleted.</summary>
    public bool IsDeleted { get; set; }
}

/// <summary>An immutable version of a file's content.</summary>
public sealed class FileVersion
{
    /// <summary>Id.</summary>
    public Guid Id { get; set; } = Guid.NewGuid();

    /// <summary>File.</summary>
    public Guid FileId { get; set; }

    /// <summary>Version number (1-based, increasing).</summary>
    public int Number { get; set; }

    /// <summary>Object store key of the content.</summary>
    public string ObjectKey { get; set; } = "";

    /// <summary>Size in bytes.</summary>
    public long Size { get; set; }

    /// <summary>SHA-256 (hex) of the content.</summary>
    public string Sha256 { get; set; } = "";

    /// <summary>Author.</summary>
    public Guid CreatedBy { get; set; }

    /// <summary>Creation time (UTC).</summary>
    public DateTime CreatedAt { get; set; }

    /// <summary>Optional note (e.g. "Restored from v2", list of collaborative editors).</summary>
    public string? Note { get; set; }
}

/// <summary>A grant of a role on a file or folder, either to a user or as a (possibly expiring) link.</summary>
public sealed class Share
{
    /// <summary>Id.</summary>
    public Guid Id { get; set; } = Guid.NewGuid();

    /// <summary>Kind of resource.</summary>
    public ResourceKind ResourceKind { get; set; }

    /// <summary>File or folder id.</summary>
    public Guid ResourceId { get; set; }

    /// <summary>Grantee (null for link shares).</summary>
    public Guid? GranteeUserId { get; set; }

    /// <summary>Granted role (never <see cref="ShareRole.Owner"/>).</summary>
    public ShareRole Role { get; set; }

    /// <summary>Secret link token (null for user shares).</summary>
    public string? LinkToken { get; set; }

    /// <summary>Expiry (UTC); null = no expiry.</summary>
    public DateTime? ExpiresAt { get; set; }

    /// <summary>Creator.</summary>
    public Guid CreatedBy { get; set; }

    /// <summary>Creation time (UTC).</summary>
    public DateTime CreatedAt { get; set; }
}

/// <summary>One audit log record.</summary>
public sealed class AuditEntry
{
    /// <summary>Id.</summary>
    public long Id { get; set; }

    /// <summary>Time (UTC).</summary>
    public DateTime At { get; set; }

    /// <summary>Actor: internal user id, host user id or "link:&lt;share id&gt;".</summary>
    public string Actor { get; set; } = "";

    /// <summary>Actor display name.</summary>
    public string? ActorName { get; set; }

    /// <summary>Action.</summary>
    public AuditAction Action { get; set; }

    /// <summary>Host name ("local" for BlueOffice-managed files).</summary>
    public string Host { get; set; } = "local";

    /// <summary>Resource kind.</summary>
    public ResourceKind ResourceKind { get; set; }

    /// <summary>Resource id (string because host ids are opaque).</summary>
    public string ResourceId { get; set; } = "";

    /// <summary>Free-form detail (JSON or text).</summary>
    public string? Detail { get; set; }

    /// <summary>Client IP address.</summary>
    public string? IpAddress { get; set; }
}

/// <summary>A collaborative editing session of one document (one row per room lifetime).</summary>
public sealed class CollabSession
{
    /// <summary>Id.</summary>
    public Guid Id { get; set; } = Guid.NewGuid();

    /// <summary>Room key ("local/&lt;file id&gt;" or "&lt;host&gt;/&lt;file id&gt;").</summary>
    public string RoomKey { get; set; } = "";

    /// <summary>Host name.</summary>
    public string Host { get; set; } = "local";

    /// <summary>File id in the host.</summary>
    public string FileId { get; set; } = "";

    /// <summary>Start (UTC).</summary>
    public DateTime StartedAt { get; set; }

    /// <summary>End (UTC); null while active.</summary>
    public DateTime? EndedAt { get; set; }

    /// <summary>Maximum number of simultaneous participants.</summary>
    public int PeakParticipants { get; set; }

    /// <summary>Number of document updates relayed.</summary>
    public long UpdateCount { get; set; }

    /// <summary>Last version written to the host.</summary>
    public string? LastSavedVersion { get; set; }

    /// <summary>Comma-separated ids of users who edited during the session.</summary>
    public string? Editors { get; set; }
}
