using BlueOffice.Infrastructure.Jobs;

namespace BlueOffice.Api.Collab;

/// <summary>
/// Settings for running several <c>api</c> instances (docs/plan/phase-6.md "Nhiều instance"): each instance has an id,
/// carried by its session ids, and collaboration rooms are owned by one instance at a time through a lease in Redis.
/// </summary>
public sealed class InstanceOptions
{
    /// <summary>This instance's id (<c>Collab:InstanceId</c>; default: host name). Letters, digits, <c>-</c> and <c>_</c>.</summary>
    public string InstanceId { get; set; } = "";

    /// <summary><c>Local</c> (one instance, default) or <c>Redis</c> (room leases shared by every instance).</summary>
    public string Ownership { get; set; } = "Local";

    /// <summary>Lease time of a room; renewed every maintenance tick while the room is open.</summary>
    public TimeSpan LeaseTime { get; set; } = TimeSpan.FromSeconds(30);

    /// <summary>Key prefix of the room leases in Redis.</summary>
    public string KeyPrefix { get; set; } = "blueoffice:room:";

    /// <summary>The configured id, or one made from the host name.</summary>
    public string ResolvedId()
    {
        var raw = string.IsNullOrWhiteSpace(InstanceId) ? Environment.MachineName : InstanceId;
        var id = new string(raw.Where(c => char.IsAsciiLetterOrDigit(c) || c is '-' or '_').ToArray());
        return id.Length == 0 ? "api" : id;
    }

    /// <summary>Session ids carry the owning instance before this separator (<c>{instance}~{guid}</c>), so a proxy can
    /// route <c>/api/sessions/{id}</c> to the instance holding the session.</summary>
    public const char SessionSeparator = '~';
}

/// <summary>Who may host a collaboration room (one instance per room, so its updates are never split).</summary>
public interface IRoomOwnership
{
    /// <summary>This instance's id.</summary>
    string InstanceId { get; }

    /// <summary>Takes (or confirms) the lease of <paramref name="roomKey"/>; returns null when this instance owns it,
    /// else the id of the instance that does.</summary>
    Task<string?> AcquireAsync(string roomKey);

    /// <summary>Extends the lease; false when it was lost (expired and taken by another instance).</summary>
    Task<bool> RenewAsync(string roomKey);

    /// <summary>Gives the lease up (the room closed).</summary>
    Task ReleaseAsync(string roomKey);
}

/// <summary>Single instance: every room is local.</summary>
public sealed class LocalRoomOwnership(InstanceOptions options) : IRoomOwnership
{
    /// <inheritdoc />
    public string InstanceId { get; } = options.ResolvedId();

    /// <inheritdoc />
    public Task<string?> AcquireAsync(string roomKey) => Task.FromResult<string?>(null);

    /// <inheritdoc />
    public Task<bool> RenewAsync(string roomKey) => Task.FromResult(true);

    /// <inheritdoc />
    public Task ReleaseAsync(string roomKey) => Task.CompletedTask;
}

/// <summary>
/// Room leases in Redis: <c>SET key instance NX PX lease</c> to take a free room, compare-and-expire to renew,
/// compare-and-delete to release, so an instance never extends or frees a lease another one holds.
/// </summary>
public sealed class RedisRoomOwnership(IRedisCommands redis, InstanceOptions options) : IRoomOwnership
{
    /// <inheritdoc />
    public string InstanceId { get; } = options.ResolvedId();

    private string Key(string roomKey) => options.KeyPrefix + roomKey;

    /// <inheritdoc />
    public async Task<string?> AcquireAsync(string roomKey)
    {
        for (var attempt = 0; attempt < 3; attempt++)
        {
            if (await redis.StringSetIfNotExistsAsync(Key(roomKey), InstanceId, options.LeaseTime)
                || await redis.ExpireIfValueAsync(Key(roomKey), InstanceId, options.LeaseTime))
            {
                return null;
            }

            // Held by another instance (or freed in between: try again).
            if (await redis.StringGetAsync(Key(roomKey)) is { } owner && owner != InstanceId)
            {
                return owner;
            }
        }

        return await redis.StringGetAsync(Key(roomKey)) ?? "";
    }

    /// <inheritdoc />
    public Task<bool> RenewAsync(string roomKey) => redis.ExpireIfValueAsync(Key(roomKey), InstanceId, options.LeaseTime);

    /// <inheritdoc />
    public Task ReleaseAsync(string roomKey) => redis.DeleteIfValueAsync(Key(roomKey), InstanceId);
}

/// <summary>The room is hosted by another instance: the client reconnects (through the proxy) to that one.</summary>
public sealed class RoomOwnedElsewhereException(string owner)
    : Exception($"The collaboration room is hosted by instance '{owner}'.")
{
    /// <summary>WebSocket close code sent to the client (application range, docs/plan/phase-6.md).</summary>
    public const int CloseCode = 4009;

    /// <summary>Owning instance.</summary>
    public string Owner { get; } = owner;
}
