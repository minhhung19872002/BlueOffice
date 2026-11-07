using System.Security.Cryptography;
using BlueOffice.Api.Audit;
using BlueOffice.Api.Common;
using BlueOffice.Domain;
using BlueOffice.Infrastructure.Data;
using BlueOffice.Infrastructure.Storage;
using Microsoft.EntityFrameworkCore;

namespace BlueOffice.Api.Files;

/// <summary>Folders, files, versions, shares and links with a permission check on every operation.</summary>
public sealed class FileService(
    BlueOfficeDbContext db,
    IObjectStore store,
    StorageOptions storage,
    AuditService audit,
    TimeProvider time)
{
    private const int MaxDepth = 64;

    private static readonly Dictionary<string, string> ContentTypes = new(StringComparer.OrdinalIgnoreCase)
    {
        [".docx"] = "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        [".xlsx"] = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        [".pptx"] = "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        [".pdf"] = "application/pdf",
        [".txt"] = "text/plain",
        [".md"] = "text/markdown",
        [".png"] = "image/png",
        [".jpg"] = "image/jpeg",
    };

    private DateTime Now => time.GetUtcNow().UtcDateTime;

    // ---------------------------------------------------------------- access

    /// <summary>Loads a file and checks <paramref name="op"/> (404 when no access at all, 403 when insufficient).</summary>
    public async Task<(FileItem File, ShareRole Role)> GetFileAsync(Guid userId, Guid fileId, Operation op, CancellationToken ct)
    {
        var file = await db.Files.FirstOrDefaultAsync(f => f.Id == fileId && !f.IsDeleted, ct) ?? throw ApiException.NotFound("File");
        var chain = await AncestorsAsync(file.FolderId, ct);
        var role = await RoleAsync(userId, ResourceKind.File, file.Id, file.OwnerId, chain, ct);
        Check(role, op);
        return (file, role);
    }

    /// <summary>Loads a folder and checks <paramref name="op"/>.</summary>
    public async Task<(Folder Folder, ShareRole Role)> GetFolderAsync(Guid userId, Guid folderId, Operation op, CancellationToken ct)
    {
        var chain = await AncestorsAsync(folderId, ct);
        var folder = chain[0];
        var role = await RoleAsync(userId, ResourceKind.Folder, folder.Id, folder.OwnerId, chain.Skip(1).ToList(), ct);
        Check(role, op);
        return (folder, role);
    }

    private static void Check(ShareRole role, Operation op)
    {
        if (role == ShareRole.None)
        {
            throw ApiException.NotFound();
        }

        if (!AccessEvaluator.Allows(role, op))
        {
            throw ApiException.Forbidden($"Role '{role}' does not allow '{op}'.");
        }
    }

    /// <summary>Folder chain starting at <paramref name="folderId"/> up to the top level; 404 if any is missing/deleted.</summary>
    private async Task<List<Folder>> AncestorsAsync(Guid? folderId, CancellationToken ct)
    {
        var chain = new List<Folder>();
        var id = folderId;
        while (id is { } current)
        {
            if (chain.Count >= MaxDepth)
            {
                throw ApiException.BadRequest("Folder hierarchy too deep.");
            }

            var f = await db.Folders.FirstOrDefaultAsync(x => x.Id == current, ct);
            if (f is null || f.IsDeleted)
            {
                throw ApiException.NotFound("Folder");
            }

            chain.Add(f);
            id = f.ParentId;
        }

        return chain;
    }

    private async Task<ShareRole> RoleAsync(Guid userId, ResourceKind kind, Guid id, Guid ownerId, List<Folder> ancestors, CancellationToken ct)
    {
        var owners = ancestors.Select(a => a.OwnerId).Append(ownerId).ToList();
        if (owners.Contains(userId))
        {
            return ShareRole.Owner;
        }

        var folderIds = ancestors.Select(a => a.Id).ToList();
        var shares = await db.Shares
            .Where(s => s.GranteeUserId == userId
                && ((s.ResourceKind == kind && s.ResourceId == id)
                    || (s.ResourceKind == ResourceKind.Folder && folderIds.Contains(s.ResourceId))))
            .ToListAsync(ct);
        return AccessEvaluator.EffectiveRole(userId, owners, shares, Now);
    }

    // ---------------------------------------------------------------- folders

    /// <summary>Creates a folder (top level, or inside a folder where the user is editor).</summary>
    public async Task<FolderDto> CreateFolderAsync(User user, CreateFolderRequest req, CancellationToken ct)
    {
        var role = ShareRole.Owner;
        if (req.ParentId is { } parent)
        {
            await GetFolderAsync(user.Id, parent, Operation.Edit, ct);
        }

        var f = new Folder { Name = ValidateName(req.Name), ParentId = req.ParentId, OwnerId = user.Id, CreatedAt = Now, UpdatedAt = Now };
        db.Folders.Add(f);
        await db.SaveChangesAsync(ct);
        return FolderDto.From(f, role);
    }

    /// <summary>Lists a folder.</summary>
    public async Task<ListingDto> ListFolderAsync(Guid userId, Guid folderId, CancellationToken ct)
    {
        var (folder, role) = await GetFolderAsync(userId, folderId, Operation.Read, ct);
        var folders = await db.Folders.Where(f => f.ParentId == folderId && !f.IsDeleted).OrderBy(f => f.Name).ToListAsync(ct);
        var files = await db.Files.Where(f => f.FolderId == folderId && !f.IsDeleted).OrderBy(f => f.Name).ToListAsync(ct);
        return new ListingDto(
            FolderDto.From(folder, role),
            folders.Select(f => FolderDto.From(f, f.OwnerId == userId ? ShareRole.Owner : role)).ToList(),
            files.Select(f => FileDto.From(f, f.OwnerId == userId ? ShareRole.Owner : role)).ToList());
    }

    /// <summary>Top level of the user's own drive.</summary>
    public async Task<ListingDto> ListRootAsync(Guid userId, CancellationToken ct)
    {
        var folders = await db.Folders.Where(f => f.ParentId == null && f.OwnerId == userId && !f.IsDeleted).OrderBy(f => f.Name).ToListAsync(ct);
        var files = await db.Files.Where(f => f.FolderId == null && f.OwnerId == userId && !f.IsDeleted).OrderBy(f => f.Name).ToListAsync(ct);
        return new ListingDto(null, folders.Select(f => FolderDto.From(f, ShareRole.Owner)).ToList(), files.Select(f => FileDto.From(f, ShareRole.Owner)).ToList());
    }

    /// <summary>Items shared directly with the user (non-expired).</summary>
    public async Task<ListingDto> ListSharedWithMeAsync(Guid userId, CancellationToken ct)
    {
        var shares = (await db.Shares.Where(s => s.GranteeUserId == userId).ToListAsync(ct))
            .Where(s => AccessEvaluator.IsActive(s, Now)).ToList();
        var folderIds = shares.Where(s => s.ResourceKind == ResourceKind.Folder).Select(s => s.ResourceId).ToList();
        var fileIds = shares.Where(s => s.ResourceKind == ResourceKind.File).Select(s => s.ResourceId).ToList();
        var folders = await db.Folders.Where(f => folderIds.Contains(f.Id) && !f.IsDeleted).ToListAsync(ct);
        var files = await db.Files.Where(f => fileIds.Contains(f.Id) && !f.IsDeleted).ToListAsync(ct);
        ShareRole RoleOf(Guid id) => shares.Where(s => s.ResourceId == id).Max(s => s.Role);
        return new ListingDto(
            null,
            folders.Select(f => FolderDto.From(f, RoleOf(f.Id))).ToList(),
            files.Select(f => FileDto.From(f, RoleOf(f.Id))).ToList());
    }

    /// <summary>Renames and/or moves a folder.</summary>
    public async Task<FolderDto> UpdateFolderAsync(Guid userId, Guid folderId, UpdateRequest req, CancellationToken ct)
    {
        var (folder, role) = await GetFolderAsync(userId, folderId, Operation.Edit, ct);
        if (req.Name is { } name)
        {
            folder.Name = ValidateName(name);
        }

        if (req.MoveToRoot || req.ParentId is not null)
        {
            if (role != ShareRole.Owner)
            {
                throw ApiException.Forbidden("Only the owner can move a folder.");
            }

            if (req.ParentId is { } target)
            {
                await GetFolderAsync(userId, target, Operation.Edit, ct);
                if ((await AncestorsAsync(target, ct)).Any(a => a.Id == folderId))
                {
                    throw ApiException.BadRequest("Cannot move a folder into itself.");
                }
            }

            folder.ParentId = req.MoveToRoot ? null : req.ParentId;
        }

        folder.UpdatedAt = Now;
        await db.SaveChangesAsync(ct);
        return FolderDto.From(folder, role);
    }

    /// <summary>Soft-deletes a folder (its content becomes unreachable).</summary>
    public async Task DeleteFolderAsync(User user, Guid folderId, CancellationToken ct)
    {
        var (folder, _) = await GetFolderAsync(user.Id, folderId, Operation.Delete, ct);
        folder.IsDeleted = true;
        folder.UpdatedAt = Now;
        await db.SaveChangesAsync(ct);
        await audit.RecordAsync(AuditAction.Delete, ResourceKind.Folder, folderId.ToString(), user.Id.ToString(), user.DisplayName, null, null, ct);
    }

    // ---------------------------------------------------------------- files

    /// <summary>Creates a file from a streamed body (size-limited, hashed while streaming).</summary>
    public async Task<FileDto> UploadAsync(User user, Guid? folderId, string name, Stream body, CancellationToken ct)
    {
        if (folderId is { } fid)
        {
            await GetFolderAsync(user.Id, fid, Operation.Edit, ct);
        }

        name = ValidateName(name);
        var file = new FileItem
        {
            Name = name,
            FolderId = folderId,
            OwnerId = user.Id,
            ContentType = ContentTypeOf(name),
            CreatedAt = Now,
            UpdatedAt = Now,
        };
        var version = await WriteVersionAsync(file, body, user.Id, null, ct);
        db.Files.Add(file);
        db.FileVersions.Add(version);
        await db.SaveChangesAsync(ct);
        await audit.RecordAsync(AuditAction.Upload, ResourceKind.File, file.Id.ToString(), user.Id.ToString(), user.DisplayName, name, null, ct);
        return FileDto.From(file, ShareRole.Owner);
    }

    /// <summary>Stores a new version. <paramref name="ifMatch"/>: expected current version (409 otherwise).</summary>
    public async Task<FileDto> PutContentAsync(Guid userId, string? userName, Guid fileId, Stream body, int? ifMatch, string? note, CancellationToken ct)
    {
        var (file, role) = await GetFileAsync(userId, fileId, Operation.Edit, ct);
        if (ifMatch is { } expected && expected != file.CurrentVersion)
        {
            throw ApiException.Conflict($"Version conflict: current version is {file.CurrentVersion}.");
        }

        var version = await WriteVersionAsync(file, body, userId, note, ct);
        db.FileVersions.Add(version);
        try
        {
            await db.SaveChangesAsync(ct);
        }
        catch (DbUpdateException)
        {
            // Unique (FileId, Number): a concurrent writer won.
            await store.DeleteAsync(version.ObjectKey, ct);
            throw ApiException.Conflict("Version conflict: concurrent update.");
        }

        await audit.RecordAsync(AuditAction.Edit, ResourceKind.File, fileId.ToString(), userId.ToString(), userName, $"v{version.Number}", null, ct);
        return FileDto.From(file, role);
    }

    /// <summary>Opens the content of the current (or given) version; records a download when <paramref name="auditDownload"/>.</summary>
    public async Task<(FileItem File, FileVersion Version, Stream Content)> OpenContentAsync(
        Guid userId, string? userName, Guid fileId, int? versionNumber, bool auditDownload, CancellationToken ct)
    {
        var (file, _) = await GetFileAsync(userId, fileId, Operation.Download, ct);
        var (v, s) = await OpenVersionAsync(file, versionNumber, ct);
        if (auditDownload)
        {
            await audit.RecordAsync(AuditAction.Download, ResourceKind.File, fileId.ToString(), userId.ToString(), userName, $"v{v.Number}", null, ct);
        }

        return (file, v, s);
    }

    private async Task<(FileVersion Version, Stream Content)> OpenVersionAsync(FileItem file, int? versionNumber, CancellationToken ct)
    {
        var n = versionNumber ?? file.CurrentVersion;
        var v = await db.FileVersions.FirstOrDefaultAsync(x => x.FileId == file.Id && x.Number == n, ct) ?? throw ApiException.NotFound("Version");
        return (v, await store.OpenReadAsync(v.ObjectKey, ct));
    }

    /// <summary>Version history (newest first).</summary>
    public async Task<IReadOnlyList<VersionDto>> ListVersionsAsync(Guid userId, Guid fileId, CancellationToken ct)
    {
        var (file, _) = await GetFileAsync(userId, fileId, Operation.Read, ct);
        var versions = await db.FileVersions.Where(v => v.FileId == fileId).OrderByDescending(v => v.Number).ToListAsync(ct);
        return versions.Select(v => new VersionDto(v.Number, v.Size, v.Sha256, v.CreatedBy, v.CreatedAt, v.Note, v.Number == file.CurrentVersion)).ToList();
    }

    /// <summary>Restores version <paramref name="number"/> as a new version (history is never rewritten).</summary>
    public async Task<FileDto> RestoreAsync(User user, Guid fileId, int number, CancellationToken ct)
    {
        var (file, role) = await GetFileAsync(user.Id, fileId, Operation.Edit, ct);
        var old = await db.FileVersions.FirstOrDefaultAsync(v => v.FileId == fileId && v.Number == number, ct) ?? throw ApiException.NotFound("Version");
        var next = file.CurrentVersion + 1;
        var key = VersionKey(file.Id, next);
        await store.CopyAsync(old.ObjectKey, key, ct);
        db.FileVersions.Add(new FileVersion
        {
            FileId = fileId,
            Number = next,
            ObjectKey = key,
            Size = old.Size,
            Sha256 = old.Sha256,
            CreatedBy = user.Id,
            CreatedAt = Now,
            Note = $"Restored from version {number}",
        });
        file.CurrentVersion = next;
        file.Size = old.Size;
        file.UpdatedAt = Now;
        await db.SaveChangesAsync(ct);
        await audit.RecordAsync(AuditAction.Restore, ResourceKind.File, fileId.ToString(), user.Id.ToString(), user.DisplayName, $"v{number} -> v{next}", null, ct);
        return FileDto.From(file, role);
    }

    /// <summary>Renames and/or moves a file.</summary>
    public async Task<FileDto> UpdateFileAsync(User user, Guid fileId, UpdateRequest req, CancellationToken ct)
    {
        var (file, role) = await GetFileAsync(user.Id, fileId, Operation.Edit, ct);
        if (req.Name is { } name)
        {
            file.Name = ValidateName(name);
            file.ContentType = ContentTypeOf(file.Name);
        }

        if (req.ParentId is { } target)
        {
            await GetFolderAsync(user.Id, target, Operation.Edit, ct);
            file.FolderId = target;
        }
        else if (req.MoveToRoot)
        {
            if (role != ShareRole.Owner)
            {
                throw ApiException.Forbidden("Only the owner can move a file to the top level.");
            }

            file.FolderId = null;
        }

        file.UpdatedAt = Now;
        await db.SaveChangesAsync(ct);
        await audit.RecordAsync(AuditAction.Rename, ResourceKind.File, fileId.ToString(), user.Id.ToString(), user.DisplayName, file.Name, null, ct);
        return FileDto.From(file, role);
    }

    /// <summary>Soft-deletes a file.</summary>
    public async Task DeleteFileAsync(User user, Guid fileId, CancellationToken ct)
    {
        var (file, _) = await GetFileAsync(user.Id, fileId, Operation.Delete, ct);
        file.IsDeleted = true;
        file.UpdatedAt = Now;
        await db.SaveChangesAsync(ct);
        await audit.RecordAsync(AuditAction.Delete, ResourceKind.File, fileId.ToString(), user.Id.ToString(), user.DisplayName, file.Name, null, ct);
    }

    // ---------------------------------------------------------------- shares & links

    private async Task CheckOwnerAsync(Guid userId, ResourceKind kind, Guid id, CancellationToken ct)
    {
        if (kind == ResourceKind.File)
        {
            await GetFileAsync(userId, id, Operation.ManageShares, ct);
        }
        else
        {
            await GetFolderAsync(userId, id, Operation.ManageShares, ct);
        }
    }

    /// <summary>Shares and links of a resource (owner only).</summary>
    public async Task<IReadOnlyList<ShareDto>> ListSharesAsync(Guid userId, ResourceKind kind, Guid id, CancellationToken ct)
    {
        await CheckOwnerAsync(userId, kind, id, ct);
        var shares = await db.Shares.Where(s => s.ResourceKind == kind && s.ResourceId == id).OrderBy(s => s.CreatedAt).ToListAsync(ct);
        return shares.Select(ShareDto.From).ToList();
    }

    /// <summary>Grants (or changes) a role to another user.</summary>
    public async Task<ShareDto> ShareAsync(User owner, ResourceKind kind, Guid id, User grantee, ShareRole role, CancellationToken ct)
    {
        if (role is ShareRole.None or ShareRole.Owner)
        {
            throw ApiException.BadRequest("Role must be viewer, commenter or editor.");
        }

        await CheckOwnerAsync(owner.Id, kind, id, ct);
        if (grantee.Id == owner.Id)
        {
            throw ApiException.BadRequest("Cannot share with yourself.");
        }

        var share = await db.Shares.FirstOrDefaultAsync(s => s.ResourceKind == kind && s.ResourceId == id && s.GranteeUserId == grantee.Id, ct);
        if (share is null)
        {
            share = new Share { ResourceKind = kind, ResourceId = id, GranteeUserId = grantee.Id, CreatedBy = owner.Id, CreatedAt = Now };
            db.Shares.Add(share);
        }

        share.Role = role;
        await db.SaveChangesAsync(ct);
        await audit.RecordAsync(AuditAction.Share, kind, id.ToString(), owner.Id.ToString(), owner.DisplayName, $"{grantee.Id}:{role}", null, ct);
        return ShareDto.From(share);
    }

    /// <summary>Creates a secret link (optionally expiring).</summary>
    public async Task<ShareDto> CreateLinkAsync(User owner, Guid fileId, CreateLinkRequest req, CancellationToken ct)
    {
        if (req.Role is ShareRole.None or ShareRole.Owner)
        {
            throw ApiException.BadRequest("Role must be viewer, commenter or editor.");
        }

        if (req.ExpiresAt is { } exp && exp.ToUniversalTime() <= Now)
        {
            throw ApiException.BadRequest("Expiry must be in the future.");
        }

        await CheckOwnerAsync(owner.Id, ResourceKind.File, fileId, ct);
        var share = new Share
        {
            ResourceKind = ResourceKind.File,
            ResourceId = fileId,
            Role = req.Role,
            LinkToken = Base64Url(RandomNumberGenerator.GetBytes(32)),
            ExpiresAt = req.ExpiresAt?.ToUniversalTime(),
            CreatedBy = owner.Id,
            CreatedAt = Now,
        };
        db.Shares.Add(share);
        await db.SaveChangesAsync(ct);
        await audit.RecordAsync(AuditAction.Share, ResourceKind.File, fileId.ToString(), owner.Id.ToString(), owner.DisplayName, $"link:{req.Role}", null, ct);
        return ShareDto.From(share);
    }

    /// <summary>Removes a share or link (owner of the resource only).</summary>
    public async Task DeleteShareAsync(User user, Guid shareId, CancellationToken ct)
    {
        var share = await db.Shares.FirstOrDefaultAsync(s => s.Id == shareId, ct) ?? throw ApiException.NotFound("Share");
        await CheckOwnerAsync(user.Id, share.ResourceKind, share.ResourceId, ct);
        db.Shares.Remove(share);
        await db.SaveChangesAsync(ct);
        await audit.RecordAsync(AuditAction.Share, share.ResourceKind, share.ResourceId.ToString(), user.Id.ToString(), user.DisplayName, $"removed:{share.Id}", null, ct);
    }

    /// <summary>Resolves a link token: 404 if unknown, 410 if expired.</summary>
    public async Task<(Share Share, FileItem File)> ResolveLinkAsync(string token, CancellationToken ct)
    {
        var share = await db.Shares.FirstOrDefaultAsync(s => s.LinkToken == token, ct) ?? throw ApiException.NotFound("Link");
        if (!AccessEvaluator.IsActive(share, Now))
        {
            throw new ApiException(StatusCodes.Status410Gone, "Link expired.");
        }

        var file = await db.Files.FirstOrDefaultAsync(f => f.Id == share.ResourceId && !f.IsDeleted, ct) ?? throw ApiException.NotFound("File");
        await AncestorsAsync(file.FolderId, ct);
        return (share, file);
    }

    /// <summary>Downloads through a link.</summary>
    public async Task<(FileItem File, Stream Content)> OpenLinkContentAsync(string token, CancellationToken ct)
    {
        var (share, file) = await ResolveLinkAsync(token, ct);
        var (_, s) = await OpenVersionAsync(file, null, ct);
        await audit.RecordAsync(AuditAction.Download, ResourceKind.File, file.Id.ToString(), $"link:{share.Id}", null, null, null, ct);
        return (file, s);
    }

    // ---------------------------------------------------------------- audit

    /// <summary>Audit log of a file (owner only).</summary>
    public async Task<IReadOnlyList<AuditDto>> GetAuditAsync(Guid userId, Guid fileId, CancellationToken ct)
    {
        await GetFileAsync(userId, fileId, Operation.ReadAudit, ct);
        var id = fileId.ToString();
        var entries = await db.AuditEntries.Where(a => a.Host == "local" && a.ResourceId == id)
            .OrderByDescending(a => a.Id).Take(500).ToListAsync(ct);
        return entries.Select(a => new AuditDto(a.At, a.Actor, a.ActorName, a.Action, a.ResourceId, a.Detail, a.IpAddress)).ToList();
    }

    /// <summary>Records a client-reported action (print, export, open).</summary>
    public async Task RecordClientActionAsync(User user, Guid fileId, ClientAuditRequest req, CancellationToken ct)
    {
        if (req.Action is not (AuditAction.Print or AuditAction.Export or AuditAction.Open or AuditAction.Download))
        {
            throw ApiException.BadRequest("Only open, print, export and download can be reported.");
        }

        await GetFileAsync(user.Id, fileId, Operation.Read, ct);
        await audit.RecordAsync(req.Action, ResourceKind.File, fileId.ToString(), user.Id.ToString(), user.DisplayName, req.Detail, null, ct);
    }

    // ---------------------------------------------------------------- helpers

    private static string VersionKey(Guid fileId, int n) => $"files/{fileId:N}/v{n}";

    private async Task<FileVersion> WriteVersionAsync(FileItem file, Stream body, Guid userId, string? note, CancellationToken ct)
    {
        var next = file.CurrentVersion + 1;
        var key = VersionKey(file.Id, next);
        using var hashing = new HashingStream(body, storage.MaxUploadBytes);
        await store.PutAsync(key, hashing, file.ContentType, ct);
        var version = new FileVersion
        {
            FileId = file.Id,
            Number = next,
            ObjectKey = key,
            Size = hashing.BytesRead,
            Sha256 = hashing.HashHex(),
            CreatedBy = userId,
            CreatedAt = Now,
            Note = note,
        };
        file.CurrentVersion = next;
        file.Size = version.Size;
        file.UpdatedAt = Now;
        return version;
    }

    /// <summary>Validates a user-supplied file/folder name (no path separators, reserved or control characters).</summary>
    public static string ValidateName(string? name)
    {
        var n = (name ?? "").Trim();
        if (n.Length is 0 or > 255 || n is "." or ".."
            || n.Any(c => char.IsControl(c) || c is '/' or '\\' or ':' or '*' or '?' or '"' or '<' or '>' or '|'))
        {
            throw ApiException.BadRequest("Invalid name.");
        }

        return n;
    }

    /// <summary>MIME type from the extension (client-supplied types are not trusted).</summary>
    public static string ContentTypeOf(string name) =>
        ContentTypes.TryGetValue(Path.GetExtension(name), out var t) ? t : "application/octet-stream";

    private static string Base64Url(byte[] bytes) =>
        Convert.ToBase64String(bytes).TrimEnd('=').Replace('+', '-').Replace('/', '_');
}

/// <summary>Read-only pass-through stream computing SHA-256 and enforcing a size limit.</summary>
public sealed class HashingStream(Stream inner, long limit) : Stream
{
    private readonly IncrementalHash _hash = IncrementalHash.CreateHash(HashAlgorithmName.SHA256);
    private byte[]? _digest;

    /// <summary>Bytes read so far.</summary>
    public long BytesRead { get; private set; }

    /// <summary>Hex digest (valid after the stream was fully read).</summary>
    public string HashHex() => Convert.ToHexString(_digest ??= _hash.GetHashAndReset()).ToLowerInvariant();

    /// <inheritdoc />
    public override bool CanRead => true;

    /// <inheritdoc />
    public override bool CanSeek => false;

    /// <inheritdoc />
    public override bool CanWrite => false;

    /// <inheritdoc />
    public override long Length => throw new NotSupportedException();

    /// <inheritdoc />
    public override long Position { get => BytesRead; set => throw new NotSupportedException(); }

    /// <inheritdoc />
    public override int Read(byte[] buffer, int offset, int count) => Track(buffer.AsSpan(offset, inner.Read(buffer, offset, count)));

    /// <inheritdoc />
    public override async ValueTask<int> ReadAsync(Memory<byte> buffer, CancellationToken cancellationToken = default)
    {
        var n = await inner.ReadAsync(buffer, cancellationToken);
        return Track(buffer.Span[..n]);
    }

    /// <inheritdoc />
    public override Task<int> ReadAsync(byte[] buffer, int offset, int count, CancellationToken cancellationToken) =>
        ReadAsync(buffer.AsMemory(offset, count), cancellationToken).AsTask();

    private int Track(ReadOnlySpan<byte> data)
    {
        BytesRead += data.Length;
        if (BytesRead > limit)
        {
            throw new PayloadTooLargeException(limit);
        }

        _hash.AppendData(data);
        return data.Length;
    }

    /// <inheritdoc />
    public override void Flush()
    {
    }

    /// <inheritdoc />
    public override long Seek(long offset, SeekOrigin origin) => throw new NotSupportedException();

    /// <inheritdoc />
    public override void SetLength(long value) => throw new NotSupportedException();

    /// <inheritdoc />
    public override void Write(byte[] buffer, int offset, int count) => throw new NotSupportedException();

    /// <inheritdoc />
    protected override void Dispose(bool disposing)
    {
        if (disposing)
        {
            _hash.Dispose();
        }

        base.Dispose(disposing);
    }
}
