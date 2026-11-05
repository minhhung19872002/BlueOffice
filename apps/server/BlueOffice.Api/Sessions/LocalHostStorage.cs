using BlueOffice.Api.Common;
using BlueOffice.Api.Files;
using BlueOffice.Domain;
using BlueOffice.Infrastructure.Data;
using Microsoft.EntityFrameworkCore;

namespace BlueOffice.Api.Sessions;

/// <summary>
/// <see cref="IHostStorage"/> over BlueOffice's own file store, so local files use the same editor-session / collaboration
/// code path as BOHP and WOPI hosts. The "access token" is the internal user id (never supplied by a client).
/// </summary>
public sealed class LocalHostStorage(IServiceScopeFactory scopes) : IHostStorage
{
    /// <summary>Pseudo host for local files.</summary>
    public static readonly HostConfig Host = new() { Name = "local", Protocol = HostProtocol.Local };

    /// <inheritdoc />
    public HostProtocol Protocol => HostProtocol.Local;

    /// <inheritdoc />
    public Task<HostFileInfo> GetFileAsync(HostCallContext ctx, string fileId, CancellationToken ct) => RunAsync(ctx, fileId, async (sp, user, id) =>
    {
        var (file, role) = await sp.GetRequiredService<FileService>().GetFileAsync(user.Id, id, Operation.Read, ct);
        return new HostFileInfo(
            file.Id.ToString(), file.Name, file.Size, file.CurrentVersion.ToString(System.Globalization.CultureInfo.InvariantCulture),
            file.OwnerId.ToString(), HostPermissions.FromRole(role), new HostUser(user.Id.ToString(), user.DisplayName, null),
            null, file.UpdatedAt, null, SupportsLocks: false, SupportsVersions: true);
    }, ct);

    /// <inheritdoc />
    public Task<HostContent> GetContentsAsync(HostCallContext ctx, string fileId, CancellationToken ct) => RunAsync(ctx, fileId, async (sp, user, id) =>
    {
        var (_, v, s) = await sp.GetRequiredService<FileService>().OpenContentAsync(user.Id, user.DisplayName, id, null, auditDownload: false, ct);
        await using (s)
        {
            using var ms = new MemoryStream();
            await s.CopyToAsync(ms, ct);
            return new HostContent(ms.ToArray(), v.Number.ToString(System.Globalization.CultureInfo.InvariantCulture));
        }
    }, ct);

    /// <inheritdoc />
    public Task<HostPutResult> PutContentsAsync(HostCallContext ctx, string fileId, byte[] content, HostPutOptions options, CancellationToken ct) =>
        RunAsync(ctx, fileId, async (sp, user, id) =>
        {
            int? ifMatch = options.IfMatchVersion is { } v ? int.Parse(v, System.Globalization.CultureInfo.InvariantCulture) : null;
            try
            {
                var dto = await sp.GetRequiredService<FileService>().PutContentAsync(user.Id, user.DisplayName, id, new MemoryStream(content), ifMatch, options.Note, ct);
                return new HostPutResult(dto.Version.ToString(System.Globalization.CultureInfo.InvariantCulture));
            }
            catch (ApiException e) when (e.Status == StatusCodes.Status409Conflict)
            {
                var current = await sp.GetRequiredService<BlueOfficeDbContext>().Files.Where(f => f.Id == id).Select(f => f.CurrentVersion).FirstAsync(ct);
                throw new HostConflictException("version", current.ToString(System.Globalization.CultureInfo.InvariantCulture), null);
            }
        }, ct);

    /// <inheritdoc />
    public Task<HostLockResult> LockAsync(HostCallContext ctx, string fileId, string lockId, TimeSpan duration, CancellationToken ct) =>
        throw new HostNotSupportedException("lock");

    /// <inheritdoc />
    public Task UnlockAsync(HostCallContext ctx, string fileId, string lockId, CancellationToken ct) =>
        throw new HostNotSupportedException("unlock");

    /// <inheritdoc />
    public Task<HostLockResult> RefreshLockAsync(HostCallContext ctx, string fileId, string lockId, TimeSpan duration, CancellationToken ct) =>
        throw new HostNotSupportedException("refresh-lock");

    /// <inheritdoc />
    public Task<IReadOnlyList<HostVersion>> GetVersionsAsync(HostCallContext ctx, string fileId, CancellationToken ct) => RunAsync(ctx, fileId, async (sp, user, id) =>
    {
        var list = await sp.GetRequiredService<FileService>().ListVersionsAsync(user.Id, id, ct);
        IReadOnlyList<HostVersion> r = list.Select(v => new HostVersion(
            v.Number.ToString(System.Globalization.CultureInfo.InvariantCulture), v.Size, v.CreatedAt,
            new HostUser(v.CreatedBy.ToString(), "", null), v.Note)).ToList();
        return r;
    }, ct);

    /// <inheritdoc />
    public Task<HostPutResult> RestoreVersionAsync(HostCallContext ctx, string fileId, string version, CancellationToken ct) => RunAsync(ctx, fileId, async (sp, user, id) =>
    {
        var n = int.TryParse(version, out var x) ? x : throw new HostException(404, "Version not found.");
        var dto = await sp.GetRequiredService<FileService>().RestoreAsync(user, id, n, ct);
        return new HostPutResult(dto.Version.ToString(System.Globalization.CultureInfo.InvariantCulture));
    }, ct);

    /// <inheritdoc />
    public Task<string> RenameAsync(HostCallContext ctx, string fileId, string newName, CancellationToken ct) => RunAsync(ctx, fileId, async (sp, user, id) =>
        (await sp.GetRequiredService<FileService>().UpdateFileAsync(user, id, new UpdateRequest(newName, null), ct)).Name, ct);

    /// <inheritdoc />
    public Task<HostCreatedFile> CreateFileAsync(HostCallContext ctx, string folderId, string name, byte[] content, CancellationToken ct) =>
        RunAsync(ctx, null, async (sp, user, _) =>
        {
            Guid? folder = Guid.TryParse(folderId, out var g) ? g : null;
            var dto = await sp.GetRequiredService<FileService>().UploadAsync(user, folder, name, new MemoryStream(content), ct);
            return new HostCreatedFile(dto.Id.ToString(), dto.Name, dto.Version.ToString(System.Globalization.CultureInfo.InvariantCulture));
        }, ct);

    private async Task<T> RunAsync<T>(HostCallContext ctx, string? fileId, Func<IServiceProvider, User, Guid, Task<T>> action, CancellationToken ct)
    {
        Guid id = Guid.Empty;
        if (fileId is not null && !Guid.TryParse(fileId, out id))
        {
            throw new HostException(404, "File not found.");
        }

        await using var scope = scopes.CreateAsyncScope();
        var userId = Guid.Parse(ctx.AccessToken);
        var user = await scope.ServiceProvider.GetRequiredService<BlueOfficeDbContext>().Users.FirstOrDefaultAsync(u => u.Id == userId, ct)
            ?? throw new HostException(403, "Unknown user.");
        try
        {
            return await action(scope.ServiceProvider, user, id);
        }
        catch (ApiException e)
        {
            throw new HostException(e.Status, e.Message);
        }
    }
}
