using BlueOffice.Domain;
using BlueOffice.Infrastructure.Data;
using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.DependencyInjection;

namespace BlueOffice.Infrastructure.Storage;

/// <summary>
/// One object of the collaboration update log (table <c>CollabLog</c>): <c>collab/&lt;room hash&gt;/meta.json</c>,
/// <c>…/snapshot.bin</c>, <c>…/u-&lt;sequence&gt;.bin</c>. Kept in the database so the log of unsaved changes survives an api
/// restart together with the rest of the server state (PostgreSQL in production).
/// </summary>
public sealed class CollabLogEntry
{
    /// <summary>Full object key (primary key).</summary>
    public string Key { get; set; } = "";

    /// <summary>Room prefix (<c>collab/&lt;hash&gt;/</c>) for listing.</summary>
    public string Room { get; set; } = "";

    /// <summary>Bytes.</summary>
    public byte[] Data { get; set; } = [];

    /// <summary>MIME type.</summary>
    public string ContentType { get; set; } = "application/octet-stream";

    /// <summary>Last write (UTC).</summary>
    public DateTime UpdatedAt { get; set; }
}

/// <summary>
/// <see cref="IObjectStore"/> over the <c>CollabLog</c> table, used for the collaboration log when
/// <c>Collab:SnapshotStore</c> = <c>Database</c> (default). Each call runs in its own DbContext scope.
/// </summary>
public sealed class DatabaseObjectStore(IServiceScopeFactory scopes, TimeProvider time) : IObjectStore
{
    /// <summary>Room prefix of a key: everything up to and including the second '/' (<c>collab/&lt;hash&gt;/</c>).</summary>
    public static string RoomOf(string key)
    {
        var first = key.IndexOf('/', StringComparison.Ordinal);
        var second = first < 0 ? -1 : key.IndexOf('/', first + 1);
        return second < 0 ? key : key[..(second + 1)];
    }

    private static bool IsRoomPrefix(string prefix) => prefix.Count(c => c == '/') == 2 && prefix.EndsWith('/') && RoomOf(prefix) == prefix;

    /// <inheritdoc />
    public async Task<long> PutAsync(string key, Stream content, string contentType, CancellationToken ct)
    {
        ObjectKeys.Check(key);
        using var ms = new MemoryStream();
        await content.CopyToAsync(ms, ct);
        await using var scope = scopes.CreateAsyncScope();
        var db = scope.ServiceProvider.GetRequiredService<BlueOfficeDbContext>();
        var row = await db.CollabLog.FindAsync([key], ct);
        if (row is null)
        {
            row = new CollabLogEntry { Key = key, Room = RoomOf(key) };
            db.CollabLog.Add(row);
        }

        row.Data = ms.ToArray();
        row.ContentType = contentType;
        row.UpdatedAt = time.GetUtcNow().UtcDateTime;
        await db.SaveChangesAsync(ct);
        return row.Data.Length;
    }

    /// <inheritdoc />
    public async Task<Stream> OpenReadAsync(string key, CancellationToken ct)
    {
        await using var scope = scopes.CreateAsyncScope();
        var db = scope.ServiceProvider.GetRequiredService<BlueOfficeDbContext>();
        var data = await db.CollabLog.AsNoTracking().Where(x => x.Key == key).Select(x => x.Data).FirstOrDefaultAsync(ct);
        return data is null ? throw new ObjectNotFoundException(key) : new MemoryStream(data, writable: false);
    }

    /// <inheritdoc />
    public async Task<bool> ExistsAsync(string key, CancellationToken ct)
    {
        await using var scope = scopes.CreateAsyncScope();
        return await scope.ServiceProvider.GetRequiredService<BlueOfficeDbContext>().CollabLog.AnyAsync(x => x.Key == key, ct);
    }

    /// <inheritdoc />
    public async Task DeleteAsync(string key, CancellationToken ct)
    {
        await using var scope = scopes.CreateAsyncScope();
        await scope.ServiceProvider.GetRequiredService<BlueOfficeDbContext>().CollabLog.Where(x => x.Key == key).ExecuteDeleteAsync(ct);
    }

    /// <inheritdoc />
    public async Task CopyAsync(string sourceKey, string destinationKey, CancellationToken ct)
    {
        await using var s = await OpenReadAsync(sourceKey, ct);
        await PutAsync(destinationKey, s, "application/octet-stream", ct);
    }

    /// <inheritdoc />
    public async Task<IReadOnlyList<string>> ListAsync(string prefix, CancellationToken ct)
    {
        await using var scope = scopes.CreateAsyncScope();
        var db = scope.ServiceProvider.GetRequiredService<BlueOfficeDbContext>();

        // Index on Room: room prefixes are matched exactly, anything else by key prefix.
        var q = db.CollabLog.AsNoTracking();
        q = IsRoomPrefix(prefix) ? q.Where(x => x.Room == prefix) : q.Where(x => x.Key.StartsWith(prefix));
        var keys = await q.Select(x => x.Key).ToListAsync(ct);
        return [.. keys.Where(k => k.StartsWith(prefix, StringComparison.Ordinal)).Order(StringComparer.Ordinal)];
    }
}
