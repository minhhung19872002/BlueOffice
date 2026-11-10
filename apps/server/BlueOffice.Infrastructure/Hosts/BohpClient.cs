using System.Net;
using System.Net.Http.Headers;
using System.Net.Http.Json;
using System.Text.Json;
using BlueOffice.Domain;

namespace BlueOffice.Infrastructure.Hosts;

/// <summary>
/// Client of the BlueOffice Host Protocol (docs/bohp.md). BOHP maps 1:1 onto <see cref="IHostStorage"/> (§A) and
/// <see cref="IHostBrowser"/> (§A2, optional browsing endpoints).
/// </summary>
public interface IBohpClient : IHostStorage, IHostBrowser;

/// <summary>HTTP implementation of <see cref="IBohpClient"/> (named HttpClient <see cref="HttpClientName"/>).</summary>
public sealed class BohpClient(IHttpClientFactory httpFactory) : IBohpClient
{
    /// <summary>Name of the HttpClient.</summary>
    public const string HttpClientName = "bohp";

    private static readonly JsonSerializerOptions Json = new(JsonSerializerDefaults.Web);

    /// <inheritdoc />
    public HostProtocol Protocol => HostProtocol.Bohp;

    /// <inheritdoc />
    public async Task<HostFileInfo> GetFileAsync(HostCallContext ctx, string fileId, CancellationToken ct)
    {
        using var res = await SendAsync(ctx, HttpMethod.Get, FilePath(fileId), null, ct);
        var dto = await ReadAsync<FileDto>(res, ct);
        var p = dto.Permissions ?? new PermissionsDto();
        return new HostFileInfo(
            dto.Id ?? fileId,
            dto.Name ?? "",
            dto.Size,
            dto.Version ?? "",
            dto.OwnerId,
            new HostPermissions(p.View, p.Comment, p.Edit, p.Share, p.Download),
            ToUser(dto.User) ?? new HostUser("", "", null),
            dto.ShareUrl,
            dto.LastModified,
            dto.Lock?.LockId,
            SupportsLocks: true,
            SupportsVersions: true)
        {
            FolderId = dto.FolderId,
        };
    }

    /// <inheritdoc />
    public async Task<HostContent> GetContentsAsync(HostCallContext ctx, string fileId, CancellationToken ct)
    {
        using var res = await SendAsync(ctx, HttpMethod.Get, FilePath(fileId) + "/contents", null, ct);
        var bytes = await res.Content.ReadAsByteArrayAsync(ct);
        return new HostContent(bytes, VersionOf(res));
    }

    /// <inheritdoc />
    public async Task<HostPutResult> PutContentsAsync(HostCallContext ctx, string fileId, byte[] content, HostPutOptions options, CancellationToken ct)
    {
        using var body = new ByteArrayContent(content);
        body.Headers.ContentType = new MediaTypeHeaderValue("application/octet-stream");
        using var res = await SendAsync(ctx, HttpMethod.Post, FilePath(fileId) + "/contents", body, ct, req =>
        {
            if (options.IfMatchVersion is { } v)
            {
                req.Headers.IfMatch.Add(new EntityTagHeaderValue(Quote(v)));
            }

            if (options.LockId is { } l)
            {
                req.Headers.Add("X-BOHP-Lock", l);
            }

            if (options.Note is { } n)
            {
                req.Headers.Add("X-BOHP-Note", Uri.EscapeDataString(n));
            }

            if (options.Editors.Count > 0)
            {
                req.Headers.Add("X-BOHP-Editors", string.Join(',', options.Editors.Select(Uri.EscapeDataString)));
            }
        });
        var dto = await ReadAsync<VersionDto>(res, ct);
        return new HostPutResult(dto.Version ?? VersionOf(res) ?? "");
    }

    /// <inheritdoc />
    public async Task<HostLockResult> LockAsync(HostCallContext ctx, string fileId, string lockId, TimeSpan duration, CancellationToken ct) =>
        await LockOpAsync(ctx, fileId, "lock", lockId, duration, ct);

    /// <inheritdoc />
    public async Task UnlockAsync(HostCallContext ctx, string fileId, string lockId, CancellationToken ct)
    {
        using var body = JsonContent.Create(new LockRequest(lockId, null), options: Json);
        using var res = await SendAsync(ctx, HttpMethod.Post, FilePath(fileId) + "/unlock", body, ct);
    }

    /// <inheritdoc />
    public async Task<HostLockResult> RefreshLockAsync(HostCallContext ctx, string fileId, string lockId, TimeSpan duration, CancellationToken ct) =>
        await LockOpAsync(ctx, fileId, "refresh-lock", lockId, duration, ct);

    /// <inheritdoc />
    public async Task<IReadOnlyList<HostVersion>> GetVersionsAsync(HostCallContext ctx, string fileId, CancellationToken ct)
    {
        using var res = await SendAsync(ctx, HttpMethod.Get, FilePath(fileId) + "/versions", null, ct);
        var dto = await ReadAsync<VersionsDto>(res, ct);
        return (dto.Versions ?? [])
            .Select(v => new HostVersion(v.Version ?? "", v.Size, v.ModifiedAt, ToUser(v.ModifiedBy), v.Note))
            .ToList();
    }

    /// <inheritdoc />
    public async Task<HostContent> GetVersionContentsAsync(HostCallContext ctx, string fileId, string version, CancellationToken ct)
    {
        using var res = await SendAsync(ctx, HttpMethod.Get, $"{FilePath(fileId)}/versions/{Uri.EscapeDataString(version)}/contents", null, ct);
        var bytes = await res.Content.ReadAsByteArrayAsync(ct);
        return new HostContent(bytes, version);
    }

    /// <inheritdoc />
    public async Task<HostPutResult> RestoreVersionAsync(HostCallContext ctx, string fileId, string version, CancellationToken ct)
    {
        using var res = await SendAsync(ctx, HttpMethod.Post, $"{FilePath(fileId)}/versions/{Uri.EscapeDataString(version)}/restore", null, ct);
        var dto = await ReadAsync<VersionDto>(res, ct);
        return new HostPutResult(dto.Version ?? "");
    }

    /// <inheritdoc />
    public async Task<string> RenameAsync(HostCallContext ctx, string fileId, string newName, CancellationToken ct)
    {
        using var body = JsonContent.Create(new RenameDto(newName), options: Json);
        using var res = await SendAsync(ctx, HttpMethod.Post, FilePath(fileId) + "/rename", body, ct);
        var dto = await ReadAsync<RenameDto>(res, ct);
        return dto.Name ?? newName;
    }

    /// <inheritdoc />
    public async Task<HostCreatedFile> CreateFileAsync(HostCallContext ctx, string folderId, string name, byte[] content, CancellationToken ct)
    {
        using var body = new ByteArrayContent(content);
        body.Headers.ContentType = new MediaTypeHeaderValue("application/octet-stream");
        var path = $"/folders/{Uri.EscapeDataString(folderId)}/files?name={Uri.EscapeDataString(name)}";
        using var res = await SendAsync(ctx, HttpMethod.Post, path, body, ct);
        var dto = await ReadAsync<FileDto>(res, ct);
        return new HostCreatedFile(dto.Id ?? "", dto.Name ?? name, dto.Version ?? "");
    }

    /// <inheritdoc />
    public async Task<HostUser> GetMeAsync(HostCallContext ctx, CancellationToken ct)
    {
        using var res = await SendAsync(ctx, HttpMethod.Get, "/me", null, ct);
        return ToUser(await ReadAsync<UserDto>(res, ct)) ?? new HostUser("", "", null);
    }

    /// <inheritdoc />
    public async Task<HostFolderListing> ListChildrenAsync(HostCallContext ctx, string folderId, CancellationToken ct)
    {
        using var res = await SendAsync(ctx, HttpMethod.Get, $"/folders/{Uri.EscapeDataString(folderId)}/children", null, ct);
        var dto = await ReadAsync<ListingDto>(res, ct);
        var folder = dto.Folder is { } f ? new HostFolder(f.Id ?? folderId, f.Name ?? "", f.ParentId) : new HostFolder(folderId, "", null);
        return new HostFolderListing(folder, ToItems(dto.Items));
    }

    /// <inheritdoc />
    public async Task<IReadOnlyList<HostItem>> ListSharedAsync(HostCallContext ctx, CancellationToken ct)
    {
        using var res = await SendAsync(ctx, HttpMethod.Get, "/shared", null, ct);
        return ToItems((await ReadAsync<ListingDto>(res, ct)).Items);
    }

    /// <inheritdoc />
    public async Task<IReadOnlyList<HostItem>> ListRecentAsync(HostCallContext ctx, int limit, CancellationToken ct)
    {
        using var res = await SendAsync(ctx, HttpMethod.Get, $"/me/recent?limit={limit}", null, ct);
        return ToItems((await ReadAsync<ListingDto>(res, ct)).Items);
    }

    /// <inheritdoc />
    public async Task<HostResolvedFile?> ResolvePathAsync(HostCallContext ctx, string relativePath, CancellationToken ct)
    {
        try
        {
            using var res = await SendAsync(ctx, HttpMethod.Get, "/sync/resolve?path=" + Uri.EscapeDataString(relativePath), null, ct);
            var dto = await ReadAsync<FileDto>(res, ct);
            return dto.Id is null ? null : new HostResolvedFile(dto.Id, dto.Name ?? "", dto.Version);
        }
        catch (HostException e) when (e.StatusCode == 404)
        {
            return null;
        }
    }

    private static List<HostItem> ToItems(List<ItemDto>? items) =>
        (items ?? [])
            .Where(i => !string.IsNullOrEmpty(i.Id))
            .Select(i => new HostItem(
                string.Equals(i.Kind, "folder", StringComparison.OrdinalIgnoreCase) ? HostItemKind.Folder : HostItemKind.File,
                i.Id!,
                i.Name ?? "",
                i.Size,
                i.Version,
                i.LastModified,
                i.OwnerId,
                i.OpenedAt))
            .ToList();

    private async Task<HostLockResult> LockOpAsync(HostCallContext ctx, string fileId, string op, string lockId, TimeSpan duration, CancellationToken ct)
    {
        using var body = JsonContent.Create(new LockRequest(lockId, (int)duration.TotalSeconds), options: Json);
        using var res = await SendAsync(ctx, HttpMethod.Post, $"{FilePath(fileId)}/{op}", body, ct);
        var dto = await ReadAsync<LockDto>(res, ct);
        return new HostLockResult(dto.LockId ?? lockId, dto.ExpiresAt);
    }

    private static string FilePath(string fileId) => "/files/" + Uri.EscapeDataString(fileId);

    private static string Quote(string v) => "\"" + v.Replace("\"", "", StringComparison.Ordinal) + "\"";

    private static string? VersionOf(HttpResponseMessage res) =>
        res.Headers.ETag?.Tag.Trim('"')
        ?? (res.Headers.TryGetValues("X-BOHP-Version", out var v) ? v.FirstOrDefault() : null);

    private static HostUser? ToUser(UserDto? u) => u is null ? null : new HostUser(u.Id ?? "", u.Name ?? "", u.AvatarUrl);

    private async Task<HttpResponseMessage> SendAsync(
        HostCallContext ctx,
        HttpMethod method,
        string path,
        HttpContent? body,
        CancellationToken ct,
        Action<HttpRequestMessage>? configure = null)
    {
        var http = httpFactory.CreateClient(HttpClientName);
        using var req = new HttpRequestMessage(method, new Uri(ctx.Host.BaseUrl.TrimEnd('/') + path));
        req.Headers.Authorization = new AuthenticationHeaderValue("Bearer", ctx.AccessToken);
        req.Content = body;
        configure?.Invoke(req);
        HttpResponseMessage res;
        try
        {
            res = await http.SendAsync(req, HttpCompletionOption.ResponseContentRead, ct);
        }
        catch (HttpRequestException e)
        {
            throw new HostException(0, $"Host '{ctx.Host.Name}' unreachable: {e.Message}");
        }

        if (res.IsSuccessStatusCode)
        {
            return res;
        }

        using (res)
        {
            if (res.StatusCode == HttpStatusCode.Conflict)
            {
                ErrorDto? err = null;
                try
                {
                    err = await res.Content.ReadFromJsonAsync<ErrorDto>(Json, ct);
                }
                catch (JsonException)
                {
                    // Body is optional.
                }

                var reason = err?.Error?.StartsWith("lock", StringComparison.OrdinalIgnoreCase) == true ? "lock" : "version";
                throw new HostConflictException(reason, err?.CurrentVersion, err?.LockId);
            }

            throw new HostException((int)res.StatusCode, $"Host '{ctx.Host.Name}' returned {(int)res.StatusCode} for {method} {path.Split('?')[0]}.");
        }
    }

    private static async Task<T> ReadAsync<T>(HttpResponseMessage res, CancellationToken ct)
        where T : new()
    {
        if (res.Content.Headers.ContentLength == 0)
        {
            return new T();
        }

        try
        {
            return await res.Content.ReadFromJsonAsync<T>(Json, ct) ?? new T();
        }
        catch (JsonException e)
        {
            throw new HostException(502, "Invalid BOHP response: " + e.Message);
        }
    }

    private sealed record LockRequest(string LockId, int? ExpiresInSeconds);

    private sealed class PermissionsDto
    {
        public bool View { get; set; }

        public bool Comment { get; set; }

        public bool Edit { get; set; }

        public bool Share { get; set; }

        public bool Download { get; set; }
    }

    private sealed class UserDto
    {
        public string? Id { get; set; }

        public string? Name { get; set; }

        public string? AvatarUrl { get; set; }
    }

    private sealed class LockDto
    {
        public string? LockId { get; set; }

        public DateTime? ExpiresAt { get; set; }
    }

    private sealed class FileDto
    {
        public string? Id { get; set; }

        public string? Name { get; set; }

        public long Size { get; set; }

        public string? Version { get; set; }

        public string? OwnerId { get; set; }

        public string? FolderId { get; set; }

        public DateTime? LastModified { get; set; }

        public string? ShareUrl { get; set; }

        public PermissionsDto? Permissions { get; set; }

        public UserDto? User { get; set; }

        public LockDto? Lock { get; set; }
    }

    private sealed class ItemDto
    {
        public string? Kind { get; set; }

        public string? Id { get; set; }

        public string? Name { get; set; }

        public long? Size { get; set; }

        public string? Version { get; set; }

        public DateTime? LastModified { get; set; }

        public string? OwnerId { get; set; }

        public DateTime? OpenedAt { get; set; }
    }

    private sealed class FolderDto
    {
        public string? Id { get; set; }

        public string? Name { get; set; }

        public string? ParentId { get; set; }
    }

    private sealed class ListingDto
    {
        public FolderDto? Folder { get; set; }

        public List<ItemDto>? Items { get; set; }
    }

    private sealed class VersionDto
    {
        public string? Version { get; set; }
    }

    private sealed class VersionItemDto
    {
        public string? Version { get; set; }

        public long Size { get; set; }

        public DateTime? ModifiedAt { get; set; }

        public UserDto? ModifiedBy { get; set; }

        public string? Note { get; set; }
    }

    private sealed class VersionsDto
    {
        public List<VersionItemDto>? Versions { get; set; }
    }

    private sealed class RenameDto
    {
        public RenameDto()
        {
        }

        public RenameDto(string name) => Name = name;

        public string? Name { get; set; }
    }

    private sealed class ErrorDto
    {
        public string? Error { get; set; }

        public string? CurrentVersion { get; set; }

        public string? LockId { get; set; }
    }
}
