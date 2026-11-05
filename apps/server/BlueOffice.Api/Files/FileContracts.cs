using BlueOffice.Domain;

namespace BlueOffice.Api.Files;

/// <summary>Folder as returned by the API.</summary>
public sealed record FolderDto(Guid Id, string Name, Guid? ParentId, Guid OwnerId, DateTime CreatedAt, DateTime UpdatedAt, ShareRole Role)
{
    /// <summary>Maps an entity.</summary>
    public static FolderDto From(Folder f, ShareRole role) => new(f.Id, f.Name, f.ParentId, f.OwnerId, f.CreatedAt, f.UpdatedAt, role);
}

/// <summary>File as returned by the API.</summary>
public sealed record FileDto(
    Guid Id,
    string Name,
    Guid? FolderId,
    Guid OwnerId,
    string ContentType,
    long Size,
    int Version,
    DateTime CreatedAt,
    DateTime UpdatedAt,
    ShareRole Role)
{
    /// <summary>Maps an entity.</summary>
    public static FileDto From(FileItem f, ShareRole role) =>
        new(f.Id, f.Name, f.FolderId, f.OwnerId, f.ContentType, f.Size, f.CurrentVersion, f.CreatedAt, f.UpdatedAt, role);
}

/// <summary>Folder content.</summary>
public sealed record ListingDto(FolderDto? Folder, IReadOnlyList<FolderDto> Folders, IReadOnlyList<FileDto> Files);

/// <summary>A version.</summary>
public sealed record VersionDto(int Number, long Size, string Sha256, Guid CreatedBy, DateTime CreatedAt, string? Note, bool Current);

/// <summary>A share or link.</summary>
public sealed record ShareDto(
    Guid Id,
    ResourceKind ResourceKind,
    Guid ResourceId,
    Guid? GranteeUserId,
    ShareRole Role,
    string? LinkToken,
    DateTime? ExpiresAt,
    DateTime CreatedAt)
{
    /// <summary>Maps an entity.</summary>
    public static ShareDto From(Share s) => new(s.Id, s.ResourceKind, s.ResourceId, s.GranteeUserId, s.Role, s.LinkToken, s.ExpiresAt, s.CreatedAt);
}

/// <summary>Audit entry.</summary>
public sealed record AuditDto(DateTime At, string Actor, string? ActorName, AuditAction Action, string ResourceId, string? Detail, string? IpAddress);

/// <summary>POST /api/folders.</summary>
public sealed record CreateFolderRequest(string Name, Guid? ParentId);

/// <summary>PATCH of a file or folder: rename and/or move (<c>MoveToRoot</c> moves to the top level).</summary>
public sealed record UpdateRequest(string? Name, Guid? ParentId, bool MoveToRoot = false);

/// <summary>POST …/shares: <c>User</c> = user id, OIDC subject or e-mail.</summary>
public sealed record CreateShareRequest(string User, ShareRole Role);

/// <summary>POST /api/files/{id}/links.</summary>
public sealed record CreateLinkRequest(ShareRole Role = ShareRole.Viewer, DateTime? ExpiresAt = null);

/// <summary>POST /api/files/{id}/audit (client-side actions: print, export).</summary>
public sealed record ClientAuditRequest(AuditAction Action, string? Detail);
