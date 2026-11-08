using System.Collections.Concurrent;
using System.Diagnostics.Metrics;
using System.Net.WebSockets;
using System.Text;
using BlueOffice.Api.Sessions;
using BlueOffice.Domain;
using BlueOffice.Infrastructure.Hosts;
using Microsoft.Extensions.Options;

namespace BlueOffice.Api.Collab;

/// <summary>Collaboration metrics (meter "BlueOffice").</summary>
public static class CollabMetrics
{
    private static readonly Meter Meter = new("BlueOffice");

    /// <summary>Relayed document updates.</summary>
    public static readonly Counter<long> Updates = Meter.CreateCounter<long>("blueoffice.collab.updates");

    /// <summary>Open WebSocket connections.</summary>
    public static readonly UpDownCounter<long> Connections = Meter.CreateUpDownCounter<long>("blueoffice.collab.connections");
}

/// <summary>Owns the active rooms (one per document) and the WebSocket receive loop.</summary>
public sealed class RoomManager
{
    private readonly ConcurrentDictionary<string, Lazy<Task<CollabRoom>>> _rooms = new();
    private readonly RoomServices _svc;

    /// <summary>Creates the manager.</summary>
    public RoomManager(
        CollabLogStore log,
        IHostNotifier notifier,
        IServiceScopeFactory scopes,
        TimeProvider time,
        IOptions<CollabOptions> options,
        IRoomOwnership ownership,
        ILogger<RoomManager> logger)
    {
        _svc = new RoomServices(log.Store, notifier, scopes, time, options.Value, logger);
        Ownership = ownership;
    }

    /// <summary>Room leases (one instance hosts a room at a time).</summary>
    public IRoomOwnership Ownership { get; }

    /// <summary>Options.</summary>
    public CollabOptions Options => _svc.Options;

    /// <summary>Active rooms (fully opened).</summary>
    public IReadOnlyList<CollabRoom> Rooms =>
        _rooms.Values.Where(l => l.IsValueCreated && l.Value.IsCompletedSuccessfully).Select(l => l.Value.Result).ToList();

    /// <summary>Room by key, if open.</summary>
    public CollabRoom? Find(string key) =>
        _rooms.TryGetValue(key, out var l) && l.IsValueCreated && l.Value.IsCompletedSuccessfully ? l.Value.Result : null;

    /// <summary>Runs a WebSocket participant until it disconnects.</summary>
    public async Task RunConnectionAsync(EditorSession session, WebSocket socket, CancellationToken ct)
    {
        var conn = new CollabConnection(Guid.NewGuid().ToString("N")[..12], session, socket, _svc.Time.GetUtcNow().UtcDateTime);
        using var sendCts = new CancellationTokenSource();
        var sendLoop = conn.RunSendLoopAsync(sendCts.Token);
        CollabMetrics.Connections.Add(1);
        CollabRoom? room = null;
        try
        {
            room = await JoinAsync(session, conn, ct);
            await ReceiveLoopAsync(room, conn, socket, ct);
        }
        catch (RoomOwnedElsewhereException e)
        {
            // Another instance hosts this document: the client reconnects through the proxy.
            conn.Close((WebSocketCloseStatus)RoomOwnedElsewhereException.CloseCode, e.Owner);
        }
        catch (Exception e) when (e is WebSocketException or OperationCanceledException)
        {
            // Client went away.
        }
        finally
        {
            CollabMetrics.Connections.Add(-1);
            if (room is not null)
            {
                await LeaveAsync(room, conn);
            }

            conn.Close(WebSocketCloseStatus.NormalClosure, "");
            try
            {
                await sendLoop.WaitAsync(TimeSpan.FromSeconds(5), CancellationToken.None);
            }
            catch (TimeoutException)
            {
                await sendCts.CancelAsync();
            }
        }
    }

    private async Task ReceiveLoopAsync(CollabRoom room, CollabConnection conn, WebSocket socket, CancellationToken ct)
    {
        var buffer = new byte[64 * 1024];
        using var message = new MemoryStream();
        while (socket.State == WebSocketState.Open)
        {
            var r = await socket.ReceiveAsync(buffer, ct);
            if (r.MessageType == WebSocketMessageType.Close)
            {
                return;
            }

            message.Write(buffer, 0, r.Count);
            if (message.Length > _svc.Options.MaxMessageBytes)
            {
                conn.Close(WebSocketCloseStatus.MessageTooBig, "message too big");
                return;
            }

            if (!r.EndOfMessage)
            {
                continue;
            }

            var data = message.ToArray();
            message.SetLength(0);
            if (r.MessageType == WebSocketMessageType.Binary)
            {
                await room.HandleBinaryAsync(conn, data);
            }
            else
            {
                await room.HandleTextAsync(conn, Encoding.UTF8.GetString(data));
            }
        }
    }

    /// <summary>Joins (creating/opening the room if needed). Retries when the room is closing concurrently.</summary>
    public async Task<CollabRoom> JoinAsync(EditorSession session, CollabConnection conn, CancellationToken ct)
    {
        for (var attempt = 0; attempt < 10; attempt++)
        {
            var lazy = _rooms.GetOrAdd(session.RoomKey, _ => new Lazy<Task<CollabRoom>>(() => OpenOwnedAsync(session)));
            CollabRoom room;
            try
            {
                room = await lazy.Value;
            }
            catch
            {
                _rooms.TryRemove(new KeyValuePair<string, Lazy<Task<CollabRoom>>>(session.RoomKey, lazy));
                throw;
            }

            if (await room.TryJoinAsync(conn, ct))
            {
                return room;
            }

            await room.Closed.WaitAsync(ct);
            _rooms.TryRemove(new KeyValuePair<string, Lazy<Task<CollabRoom>>>(session.RoomKey, lazy));
        }

        throw new InvalidOperationException("Could not join the collaboration room.");
    }

    /// <summary>Opens a room after taking its lease (released again if opening fails).</summary>
    private async Task<CollabRoom> OpenOwnedAsync(EditorSession session)
    {
        if (await Ownership.AcquireAsync(session.RoomKey) is { } owner)
        {
            throw new RoomOwnedElsewhereException(owner);
        }

        try
        {
            return await CollabRoom.OpenAsync(_svc, session, CancellationToken.None);
        }
        catch
        {
            await Ownership.ReleaseAsync(session.RoomKey);
            throw;
        }
    }

    /// <summary>The lease of <paramref name="room"/> was lost: its clients reconnect to the new owner.</summary>
    public void LeaseLost(CollabRoom room)
    {
        foreach (var s in room.Connections.Select(c => c.Session).Distinct().ToList())
        {
            room.Disconnect(s, (WebSocketCloseStatus)RoomOwnedElsewhereException.CloseCode, "room moved");
        }
    }

    /// <summary>Leaves; closes and forgets the room when it becomes empty.</summary>
    public async Task LeaveAsync(CollabRoom room, CollabConnection conn)
    {
        if (await room.LeaveAsync(conn))
        {
            await room.CloseAsync();
            if (_rooms.TryGetValue(room.Key, out var lazy) && lazy.IsValueCreated && lazy.Value.IsCompletedSuccessfully && lazy.Value.Result == room)
            {
                _rooms.TryRemove(new KeyValuePair<string, Lazy<Task<CollabRoom>>>(room.Key, lazy));
                await Ownership.ReleaseAsync(room.Key);
            }
        }
    }

    /// <summary>Called after a session's permissions changed: pushes <c>mode-changed</c> to its connections.</summary>
    public Task PermissionsChangedAsync(EditorSession session) =>
        Find(session.RoomKey) is { } room ? room.UpdateModesAsync() : Task.CompletedTask;
}

/// <summary>Background loop: room ticks (persistence, autosave, lock refresh), permission re-checks, session expiry.</summary>
public sealed class CollabMaintenanceService(
    RoomManager rooms,
    SessionStore sessions,
    IServiceScopeFactory scopes,
    TimeProvider time,
    ILogger<CollabMaintenanceService> logger) : BackgroundService
{
    /// <inheritdoc />
    protected override async Task ExecuteAsync(CancellationToken stoppingToken)
    {
        using var timer = new PeriodicTimer(rooms.Options.TickInterval, time);
        while (await timer.WaitForNextTickAsync(stoppingToken))
        {
            await TickAsync(stoppingToken);
        }
    }

    /// <summary>One maintenance pass (public for tests).</summary>
    public async Task TickAsync(CancellationToken ct)
    {
        var now = time.GetUtcNow().UtcDateTime;
        foreach (var room in rooms.Rooms)
        {
            try
            {
                if (!await rooms.Ownership.RenewAsync(room.Key))
                {
                    logger.LogWarning("Lost the lease of room {Room}; its clients reconnect", room.Key);
                    rooms.LeaseLost(room);
                    continue;
                }

                await room.TickAsync(ct);
                if (now - room.LastPermissionCheck >= rooms.Options.PermissionRecheckInterval)
                {
                    room.LastPermissionCheck = now;
                    await using var scope = scopes.CreateAsyncScope();
                    var svc = scope.ServiceProvider.GetRequiredService<SessionService>();
                    foreach (var s in room.Connections.Select(c => c.Session).Distinct())
                    {
                        await svc.RefreshAsync(s, null, ct);
                    }
                }
            }
            catch (Exception e) when (e is not OperationCanceledException)
            {
                logger.LogError(e, "Maintenance of room {Room} failed", room.Key);
            }
        }

        foreach (var s in sessions.PurgeExpired(now))
        {
            rooms.Find(s.RoomKey)?.Disconnect(s, (WebSocketCloseStatus)4401, "session expired");
        }
    }
}
