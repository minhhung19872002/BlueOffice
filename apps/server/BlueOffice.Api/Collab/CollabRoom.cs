using System.Diagnostics;
using System.Net.WebSockets;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using BlueOffice.Api.Audit;
using BlueOffice.Api.Sessions;
using BlueOffice.Domain;
using BlueOffice.Infrastructure.Data;
using BlueOffice.Infrastructure.Hosts;

namespace BlueOffice.Api.Collab;

/// <summary>Options of section <c>Collab</c>.</summary>
public sealed class CollabOptions
{
    /// <summary>Autosave period while the document is dirty (prompt §6A.4: 30–60 s).</summary>
    public TimeSpan AutosaveInterval { get; set; } = TimeSpan.FromSeconds(45);

    /// <summary>Quiet time after the last change before an autosave.</summary>
    public TimeSpan AutosaveDebounce { get; set; } = TimeSpan.FromSeconds(2);

    /// <summary>Maintenance tick (persistence flush, autosave, lock refresh).</summary>
    public TimeSpan TickInterval { get; set; } = TimeSpan.FromSeconds(2);

    /// <summary>Full snapshot period (incremental batches in between).</summary>
    public TimeSpan SnapshotInterval { get; set; } = TimeSpan.FromSeconds(60);

    /// <summary>How long to wait for a client checkpoint after a save-request before asking again.</summary>
    public TimeSpan SaveRequestTimeout { get; set; } = TimeSpan.FromSeconds(30);

    /// <summary>Permission re-check period (calls GetFile on the host for every session).</summary>
    public TimeSpan PermissionRecheckInterval { get; set; } = TimeSpan.FromSeconds(60);

    /// <summary>Editor session lifetime (sliding).</summary>
    public TimeSpan SessionLifetime { get; set; } = TimeSpan.FromHours(12);

    /// <summary>IANA time zone used for user-visible timestamps (conflict copy names).</summary>
    public string TimeZone { get; set; } = "Asia/Ho_Chi_Minh";

    /// <summary>Maximum size of one WebSocket message.</summary>
    public int MaxMessageBytes { get; set; } = 16 * 1024 * 1024;

    /// <summary>Update-log size that triggers a compaction (full state requested from a client).</summary>
    public long MaxLogBytes { get; set; } = 32L * 1024 * 1024;

    /// <summary>Where the update log is persisted: <c>Database</c> (default) or <c>ObjectStore</c>, see <see cref="CollabLogStore"/>.</summary>
    public string SnapshotStore { get; set; } = "Database";
}

/// <summary>
/// Where rooms keep their update log (snapshot + incremental batches + meta): <c>Collab:SnapshotStore</c> =
/// <c>Database</c> (default, table <c>CollabLog</c>) or <c>ObjectStore</c> (MinIO / file system, as before).
/// </summary>
public sealed record CollabLogStore(IObjectStore Store, string Kind)
{
    /// <summary>Builds the store from configuration.</summary>
    public static CollabLogStore Create(IServiceProvider sp, IConfiguration config)
    {
        var kind = config["Collab:SnapshotStore"] ?? "Database";
        if (kind.Equals("ObjectStore", StringComparison.OrdinalIgnoreCase))
        {
            return new CollabLogStore(sp.GetRequiredService<IObjectStore>(), "ObjectStore");
        }

        return kind.Equals("Database", StringComparison.OrdinalIgnoreCase)
            ? new CollabLogStore(new Infrastructure.Storage.DatabaseObjectStore(sp.GetRequiredService<IServiceScopeFactory>(), sp.GetRequiredService<TimeProvider>()), "Database")
            : throw new InvalidOperationException($"Unknown Collab:SnapshotStore '{kind}' (expected Database or ObjectStore).");
    }
}

/// <summary>Shared dependencies of rooms.</summary>
public sealed record RoomServices(
    IObjectStore Store,
    IHostNotifier Notifier,
    IServiceScopeFactory Scopes,
    TimeProvider Time,
    CollabOptions Options,
    ILogger Logger);

/// <summary>Outcome of a save attempt.</summary>
public enum SaveOutcome
{
    /// <summary>Written to the host.</summary>
    Saved,

    /// <summary>Nothing to save.</summary>
    UpToDate,

    /// <summary>No fresh checkpoint: a client was asked for one (save-request).</summary>
    Requested,

    /// <summary>Host reported a version/lock conflict.</summary>
    Conflict,

    /// <summary>Host error.</summary>
    Failed,
}

/// <summary>Result of a save.</summary>
public sealed record SaveResult(SaveOutcome Outcome, string? Version, string? Message);

/// <summary>
/// One collaborative document. Relays y-protocols messages between connections without interpreting Yjs updates,
/// keeps the update log (persisted to the object store), owns the host lock and saves client checkpoints to the host.
/// All state is guarded by <see cref="_gate"/>; host and storage I/O happens outside it.
/// </summary>
public sealed class CollabRoom
{
    private static readonly ActivitySource Activity = new("BlueOffice.Collab");
    private readonly SemaphoreSlim _gate = new(1, 1);
    private readonly SemaphoreSlim _saveGate = new(1, 1);
    private readonly SemaphoreSlim _persistGate = new(1, 1);
    private readonly RoomServices _svc;
    private readonly List<CollabConnection> _conns = [];
    private readonly List<CollabConnection> _pendingSync = [];
    private readonly Dictionary<ulong, AwarenessEntry> _awareness = [];
    private readonly Dictionary<string, string> _editorNames = [];
    private readonly HashSet<string> _editorsSinceSave = [];
    private readonly TaskCompletionSource _closed = new(TaskCreationOptions.RunContinuationsAsynchronously);
    private List<byte[]> _log = [];
    private int _persistedCount;
    private bool _needSnapshot;
    private long _seq;
    private string? _metaVersion;
    private DateTime _lastSnapshot;
    private (string ConnId, int Index)? _compaction;
    private string? _seederId;
    private byte[]? _checkpoint;
    private DateTime _checkpointAt;
    private EditorSession? _checkpointSession;
    private bool _checkpointSaved = true;
    private DateTime _saveRequestedAt;
    private bool _saveRequested;
    private string? _saveRequestReason;
    private EditorSession? _lockSession;
    private bool _closing;

    private CollabRoom(RoomServices svc, string key, HostConfig host, IHostStorage storage, string fileId, string baseVersion)
    {
        _svc = svc;
        Key = key;
        Host = host;
        Storage = storage;
        FileId = fileId;
        BaseVersion = baseVersion;
        var hash = Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(key))).ToLowerInvariant();
        Prefix = $"collab/{hash[..40]}/";
        var now = Now;
        LastSave = now;
        LastLockRefresh = now;
        LastPermissionCheck = now;
        _lastSnapshot = now;
    }

    /// <summary>Room key.</summary>
    public string Key { get; }

    /// <summary>Host.</summary>
    public HostConfig Host { get; }

    /// <summary>Storage adapter.</summary>
    public IHostStorage Storage { get; }

    /// <summary>File id in the host.</summary>
    public string FileId { get; }

    /// <summary>Object-store prefix of the update log.</summary>
    public string Prefix { get; }

    /// <summary>Host version the document state is based on (updated by each save).</summary>
    public string BaseVersion { get; private set; }

    /// <summary>Unsaved changes exist.</summary>
    public bool Dirty { get; private set; }

    /// <summary>Last relayed change.</summary>
    public DateTime LastChange { get; private set; }

    /// <summary>Last successful save (or room creation).</summary>
    public DateTime LastSave { get; private set; }

    /// <summary>Last save failed with a conflict; autosave paused until a new checkpoint arrives.</summary>
    public bool Conflict { get; private set; }

    /// <summary>Lock id owned by the room (shared by all participants, prompt §6A.4).</summary>
    public string LockId { get; } = Guid.NewGuid().ToString("N");

    /// <summary>The host lock is held.</summary>
    public bool LockHeld { get; private set; }

    /// <summary>The file is locked by someone else: everybody is read-only.</summary>
    public bool LockConflict { get; private set; }

    /// <summary>Last lock refresh.</summary>
    public DateTime LastLockRefresh { get; private set; }

    /// <summary>Last permission re-check.</summary>
    public DateTime LastPermissionCheck { get; set; }

    /// <summary>Id of the <see cref="CollabSession"/> row.</summary>
    public Guid SessionRowId { get; private set; }

    /// <summary>Number of relayed updates.</summary>
    public long UpdateCount { get; private set; }

    /// <summary>Completes when the room is closed.</summary>
    public Task Closed => _closed.Task;

    /// <summary>Number of entries in the update log.</summary>
    public int LogCount => _log.Count;

    /// <summary>Connections (snapshot).</summary>
    public IReadOnlyList<CollabConnection> Connections
    {
        get
        {
            lock (_conns)
            {
                return _conns.ToList();
            }
        }
    }

    private DateTime Now => _svc.Time.GetUtcNow().UtcDateTime;

    // ================================================================ lifecycle

    /// <summary>Creates a room, reloading the persisted update log when it matches the host version.</summary>
    public static async Task<CollabRoom> OpenAsync(RoomServices svc, EditorSession first, CancellationToken ct)
    {
        var room = new CollabRoom(svc, first.RoomKey, first.Host, first.Storage, first.FileId, first.File.Version);
        await room.LoadAsync(ct);
        room.SessionRowId = Guid.NewGuid();
        await using (var scope = svc.Scopes.CreateAsyncScope())
        {
            var db = scope.ServiceProvider.GetRequiredService<BlueOfficeDbContext>();
            db.CollabSessions.Add(new CollabSession
            {
                Id = room.SessionRowId,
                RoomKey = room.Key,
                Host = room.Host.Name,
                FileId = room.FileId,
                StartedAt = room.Now,
            });
            await db.SaveChangesAsync(ct);
        }

        svc.Notifier.Publish(room.Host, HostEvents.SessionStarted, room.FileId, new
        {
            sessionId = room.SessionRowId,
            startedBy = new { id = first.User.Id, name = first.User.Name },
            version = room.BaseVersion,
        });
        return room;
    }

    private async Task LoadAsync(CancellationToken ct)
    {
        var keys = await _svc.Store.ListAsync(Prefix, ct);
        if (keys.Count == 0)
        {
            return;
        }

        string? baseVersion = null;
        if (keys.Contains(Prefix + "meta.json"))
        {
            await using var s = await _svc.Store.OpenReadAsync(Prefix + "meta.json", ct);
            var meta = await JsonSerializer.DeserializeAsync<RoomMeta>(s, cancellationToken: ct);
            baseVersion = meta?.BaseVersion;
        }

        if (baseVersion != BaseVersion)
        {
            // The host content changed since the log was written: the log is stale.
            _svc.Logger.LogInformation("Discarding stale collab log of {Room} (log {LogVersion}, host {HostVersion})", Key, baseVersion, BaseVersion);
            foreach (var k in keys)
            {
                await _svc.Store.DeleteAsync(k, ct);
            }

            return;
        }

        var log = new List<byte[]>();
        foreach (var k in keys.Where(k => k == Prefix + "snapshot.bin").Concat(keys.Where(k => k.StartsWith(Prefix + "u-", StringComparison.Ordinal))))
        {
            await using var s = await _svc.Store.OpenReadAsync(k, ct);
            using var ms = new MemoryStream();
            await s.CopyToAsync(ms, ct);
            log.AddRange(YProtocol.DecodeFrames(ms.ToArray()));
            if (k.StartsWith(Prefix + "u-", StringComparison.Ordinal) && long.TryParse(k[(Prefix.Length + 2)..^4], out var seq))
            {
                _seq = Math.Max(_seq, seq);
            }
        }

        _log = log;
        _persistedCount = log.Count;
        _metaVersion = baseVersion;
    }

    /// <summary>Adds a connection; false if the room is closing (caller retries with a new room).</summary>
    public async Task<bool> TryJoinAsync(CollabConnection conn, CancellationToken ct)
    {
        await _gate.WaitAsync(ct);
        try
        {
            if (_closing)
            {
                return false;
            }

            lock (_conns)
            {
                _conns.Add(conn);
            }

            conn.Mode = ModeOf(conn.Session);
            var seed = false;
            if (_log.Count == 0 && conn.Mode == "edit" && (_seederId is null || !HasConnection(_seederId)))
            {
                _seederId = conn.Id;
                seed = true;
            }

            conn.SendEvent(new
            {
                type = "welcome",
                connectionId = conn.Id,
                sessionId = conn.Session.Id,
                mode = conn.Mode,
                seed,
                version = BaseVersion,
                user = conn.Session.User,
                locked = LockHeld,
                collaborators = Collaborators(),
            });
            if (_awareness.Count > 0)
            {
                conn.Send(YProtocol.EncodeAwareness(_awareness.Values.ToList()));
            }

            BroadcastCollaborators();
            await UpdatePeakAsync();
        }
        finally
        {
            _gate.Release();
        }

        if (conn.Mode == "edit")
        {
            await EnsureLockAsync(conn.Session, ct);
        }

        return true;
    }

    /// <summary>Removes a connection; returns true when the room became empty (caller closes it).</summary>
    public async Task<bool> LeaveAsync(CollabConnection conn)
    {
        await _gate.WaitAsync();
        try
        {
            bool removed;
            lock (_conns)
            {
                removed = _conns.Remove(conn);
            }

            if (!removed)
            {
                return false;
            }

            _pendingSync.Remove(conn);
            if (_compaction?.ConnId == conn.Id)
            {
                _compaction = null;
            }

            // Remove the awareness states announced by this connection (as y-websocket servers do).
            if (conn.AwarenessClients.Count > 0)
            {
                var removedStates = conn.AwarenessClients.Select(kv => new AwarenessEntry(kv.Key, kv.Value + 1, "null")).ToList();
                foreach (var e in removedStates)
                {
                    _awareness.Remove(e.ClientId);
                }

                Broadcast(YProtocol.EncodeAwareness(removedStates), except: null);
            }

            if (_seederId == conn.Id && _log.Count == 0)
            {
                ReassignSeeder();
            }

            BroadcastCollaborators();
            if (_conns.Count == 0)
            {
                _closing = true;
                return true;
            }

            return false;
        }
        finally
        {
            _gate.Release();
        }
    }

    /// <summary>Final save, persistence, unlock, webhook. Called once after the last participant left.</summary>
    public async Task CloseAsync()
    {
        using var activity = Activity.StartActivity("collab.close");
        try
        {
            if (_checkpoint is not null && !_checkpointSaved && !Conflict)
            {
                await SaveAsync("session-end", overwrite: false, CancellationToken.None);
            }

            bool fullySaved;
            await _gate.WaitAsync();
            try
            {
                fullySaved = !Dirty || _log.Count == 0;
            }
            finally
            {
                _gate.Release();
            }

            if (fullySaved)
            {
                // Host content is authoritative again; the next session starts from it.
                foreach (var k in await _svc.Store.ListAsync(Prefix, CancellationToken.None))
                {
                    await _svc.Store.DeleteAsync(k, CancellationToken.None);
                }
            }
            else
            {
                await PersistAsync(forceSnapshot: true, CancellationToken.None);
            }

            if (LockHeld && _lockSession is { } ls)
            {
                try
                {
                    await Storage.UnlockAsync(ls.Context, FileId, LockId, CancellationToken.None);
                }
                catch (HostException e)
                {
                    _svc.Logger.LogWarning("Unlock of {Room} failed: {Error}", Key, e.Message);
                }

                LockHeld = false;
            }

            var editors = _editorNames.Select(kv => new { id = kv.Key, name = kv.Value }).ToList();
            _svc.Notifier.Publish(Host, HostEvents.SessionEnded, FileId, new
            {
                sessionId = SessionRowId,
                version = BaseVersion,
                editors,
                unsavedChanges = !fullySaved,
            });
            await using var scope = _svc.Scopes.CreateAsyncScope();
            var db = scope.ServiceProvider.GetRequiredService<BlueOfficeDbContext>();
            if (await db.CollabSessions.FindAsync(SessionRowId) is { } row)
            {
                row.EndedAt = Now;
                row.UpdateCount = UpdateCount;
                row.Editors = string.Join(',', _editorNames.Keys);
                row.LastSavedVersion = BaseVersion;
                await db.SaveChangesAsync();
            }
        }
        catch (Exception e)
        {
            _svc.Logger.LogError(e, "Closing room {Room} failed", Key);
        }
        finally
        {
            _closed.TrySetResult();
        }
    }

    // ================================================================ messages

    /// <summary>Handles a binary y-protocols message from <paramref name="conn"/>.</summary>
    public async Task HandleBinaryAsync(CollabConnection conn, byte[] data)
    {
        await _gate.WaitAsync();
        try
        {
            var pos = 0;
            var type = YProtocol.ReadVarUint(data, ref pos);
            switch (type)
            {
                case YProtocol.MessageSync:
                    HandleSync(conn, data, pos);
                    break;
                case YProtocol.MessageAwareness:
                    HandleAwareness(conn, data, pos);
                    break;
                case YProtocol.MessageQueryAwareness:
                    conn.Send(YProtocol.EncodeAwareness(_awareness.Values.ToList()));
                    break;
                default:
                    // messageAuth and unknown types from clients are ignored.
                    break;
            }
        }
        catch (FormatException e)
        {
            _svc.Logger.LogDebug("Malformed collab message from {Conn}: {Error}", conn.Id, e.Message);
        }
        finally
        {
            _gate.Release();
        }
    }

    private void HandleSync(CollabConnection conn, byte[] data, int pos)
    {
        var syncType = YProtocol.ReadVarUint(data, ref pos);
        var payload = YProtocol.ReadVarUint8Array(data, ref pos);
        if (syncType == YProtocol.SyncStep1)
        {
            // The server has no Yjs document: answer with the whole update log (Yjs ignores what it already has).
            if (_log.Count > 0)
            {
                SendLog(conn);
            }
            else if (_seederId is { } seeder && seeder != conn.Id && HasConnection(seeder))
            {
                // Wait for the seeder's state instead of letting this client start from an empty document.
                _pendingSync.Add(conn);
            }
            else
            {
                conn.Send(YProtocol.EncodeSync(YProtocol.SyncStep2, YProtocol.EmptyUpdate));
            }

            if (_log.Count == 0 && conn.Id == _seederId && !conn.AwaitingFullState)
            {
                conn.AwaitingFullState = true;
                conn.Send(YProtocol.EncodeSync(YProtocol.SyncStep1, YProtocol.EmptyStateVector));
            }

            return;
        }

        if (syncType is not (YProtocol.SyncStep2 or YProtocol.SyncUpdate))
        {
            return;
        }

        if (conn.Mode != "edit")
        {
            // Read-only participants may not change the document: drop silently.
            return;
        }

        if (payload.Length == 0)
        {
            return;
        }

        var fullState = syncType == YProtocol.SyncStep2 && conn.AwaitingFullState;
        if (fullState && _compaction is { } c && c.ConnId == conn.Id)
        {
            // Compaction: the client's full state replaces everything logged before the request.
            var tail = _log.Skip(c.Index).ToList();
            _log = [payload, .. tail];
            _compaction = null;
            _needSnapshot = true;
            _persistedCount = 0;
            conn.AwaitingFullState = false;
            return;
        }

        if (fullState)
        {
            conn.AwaitingFullState = false;
        }

        _log.Add(payload);
        UpdateCount++;
        Dirty = true;
        Conflict = false;
        LastChange = Now;
        _editorsSinceSave.Add(conn.Session.User.Id);
        _editorNames[conn.Session.User.Id] = conn.Session.User.Name;
        Broadcast(YProtocol.EncodeSync(YProtocol.SyncUpdate, payload), except: conn);
        CollabMetrics.Updates.Add(1);

        if (_pendingSync.Count > 0)
        {
            foreach (var p in _pendingSync)
            {
                SendLog(p);
            }

            _pendingSync.Clear();
        }

        if (_compaction is null && _log.Sum(x => (long)x.Length) > _svc.Options.MaxLogBytes)
        {
            var leader = EditorConnections().FirstOrDefault();
            if (leader is not null)
            {
                _compaction = (leader.Id, _log.Count);
                leader.AwaitingFullState = true;
                leader.Send(YProtocol.EncodeSync(YProtocol.SyncStep1, YProtocol.EmptyStateVector));
            }
        }
    }

    private void HandleAwareness(CollabConnection conn, byte[] data, int pos)
    {
        var update = YProtocol.ReadVarUint8Array(data, ref pos);
        foreach (var e in YProtocol.DecodeAwareness(update))
        {
            if (e.State == "null")
            {
                conn.AwarenessClients.Remove(e.ClientId);
                _awareness.Remove(e.ClientId);
            }
            else
            {
                conn.AwarenessClients[e.ClientId] = e.Clock;
                _awareness[e.ClientId] = e;
            }
        }

        Broadcast(data, except: conn);
    }

    /// <summary>Handles a JSON control message (<c>{"type":"save"}</c>, <c>{"type":"ping"}</c>).</summary>
    public async Task HandleTextAsync(CollabConnection conn, string json)
    {
        string? type;
        try
        {
            using var doc = JsonDocument.Parse(json);
            type = doc.RootElement.TryGetProperty("type", out var t) ? t.GetString() : null;
        }
        catch (JsonException)
        {
            return;
        }

        switch (type)
        {
            case "ping":
                conn.SendEvent(new { type = "pong" });
                break;
            case "save" when conn.Mode == "edit":
                await SaveAsync("manual", overwrite: false, CancellationToken.None);
                break;
        }
    }

    // ================================================================ save

    /// <summary>Stores a document produced by a client (docx bytes); saves when asked or when a save is pending.</summary>
    public async Task<SaveResult> SetCheckpointAsync(EditorSession session, byte[] content, bool save, bool overwrite, CancellationToken ct)
    {
        bool doSave;
        string reason;
        await _gate.WaitAsync(ct);
        try
        {
            _checkpoint = content;
            _checkpointAt = Now;
            _checkpointSession = session;
            _checkpointSaved = false;
            doSave = save || overwrite || _saveRequested;

            // A checkpoint answering a save-request keeps the reason of the request (e.g. "autosave").
            reason = save || overwrite ? "manual" : _saveRequestReason ?? "requested";
            if (overwrite)
            {
                Conflict = false;
            }
        }
        finally
        {
            _gate.Release();
        }

        return doSave
            ? await SaveAsync(reason, overwrite, ct)
            : new SaveResult(SaveOutcome.UpToDate, BaseVersion, "Checkpoint stored.");
    }

    /// <summary>Saves the latest checkpoint to the host, or asks a client for a fresh one.</summary>
    public async Task<SaveResult> SaveAsync(string reason, bool overwrite, CancellationToken ct)
    {
        await _saveGate.WaitAsync(ct);
        try
        {
            byte[] content;
            EditorSession session;
            string baseVersion;
            List<string> editorIds;
            List<string> editorNames;
            string? lockId;
            await _gate.WaitAsync(ct);
            try
            {
                var stale = _checkpoint is null || (_checkpointAt < LastChange && reason != "session-end");
                if (stale)
                {
                    if (EditorConnections().FirstOrDefault() is { } leader)
                    {
                        _saveRequested = true;
                        _saveRequestedAt = Now;
                        _saveRequestReason = reason;
                        leader.SendEvent(new { type = "save-request", reason });
                        return new SaveResult(SaveOutcome.Requested, BaseVersion, null);
                    }

                    if (_checkpoint is null)
                    {
                        return new SaveResult(SaveOutcome.UpToDate, BaseVersion, "No checkpoint.");
                    }
                }

                if (_checkpointSaved)
                {
                    return new SaveResult(SaveOutcome.UpToDate, BaseVersion, null);
                }

                session = _checkpointSession is { Permissions.Edit: true } cs ? cs
                    : EditorConnections().Select(c => c.Session).FirstOrDefault()
                    ?? _checkpointSession!;
                content = _checkpoint!;
                baseVersion = BaseVersion;
                editorIds = _editorsSinceSave.Count > 0 ? _editorsSinceSave.ToList() : [session.User.Id];
                editorNames = editorIds.Select(id => _editorNames.TryGetValue(id, out var n) ? n : id).ToList();
                lockId = LockHeld ? LockId : null;
            }
            finally
            {
                _gate.Release();
            }

            var checkpointAt = _checkpointAt;
            using var activity = Activity.StartActivity("collab.save");
            var note = "Edited in BlueOffice by " + string.Join(", ", editorNames);
            HostPutResult result;
            try
            {
                result = await Storage.PutContentsAsync(
                    session.Context, FileId, content, new HostPutOptions(overwrite ? null : baseVersion, lockId, note, editorIds), ct);
            }
            catch (HostConflictException e)
            {
                await WithGateAsync(() =>
                {
                    Conflict = true;
                    Broadcast(new { type = "save-conflict", reason = e.Reason, currentVersion = e.CurrentVersion });
                });
                return new SaveResult(SaveOutcome.Conflict, e.CurrentVersion, e.Message);
            }
            catch (HostException e)
            {
                _svc.Logger.LogWarning("Save of {Room} failed: {Error}", Key, e.Message);
                await WithGateAsync(() => Broadcast(new { type = "save-error", message = e.Message }));
                return new SaveResult(SaveOutcome.Failed, null, e.Message);
            }

            var now = Now;
            await WithGateAsync(() =>
            {
                BaseVersion = result.Version;
                _checkpointSaved = true;
                Dirty = LastChange > checkpointAt;
                LastSave = now;
                _saveRequested = false;
                _saveRequestReason = null;
                Conflict = false;
                foreach (var id in editorIds)
                {
                    _editorsSinceSave.Remove(id);
                }

                foreach (var c in Connections)
                {
                    c.Session.File = c.Session.File with { Version = result.Version };
                }

                Broadcast(new { type = "saved", version = result.Version, at = now, reason, editors = editorNames });
            });
            _metaVersion = null; // meta.json must be rewritten with the new base version
            _svc.Notifier.Publish(Host, HostEvents.FileSaved, FileId, new
            {
                version = result.Version,
                reason,
                sessionId = SessionRowId,
                editors = editorIds.Zip(editorNames, (id, name) => new { id, name }).ToList(),
            });
            await using (var scope = _svc.Scopes.CreateAsyncScope())
            {
                await scope.ServiceProvider.GetRequiredService<AuditService>().RecordAsync(
                    AuditAction.Edit, ResourceKind.File, FileId, string.Join(',', editorIds), string.Join(", ", editorNames),
                    $"version {result.Version} ({reason})", Host.Protocol == HostProtocol.Local ? null : Host, ct);
            }

            return new SaveResult(SaveOutcome.Saved, result.Version, null);
        }
        finally
        {
            _saveGate.Release();
        }
    }

    // ================================================================ maintenance

    /// <summary>Periodic work: persistence, autosave, lock refresh.</summary>
    public async Task TickAsync(CancellationToken ct)
    {
        await PersistAsync(forceSnapshot: false, ct);
        var now = Now;
        var o = _svc.Options;
        if (Dirty && !Conflict && now - LastSave >= o.AutosaveInterval && now - LastChange >= o.AutosaveDebounce
            && (!_saveRequested || now - _saveRequestedAt >= o.SaveRequestTimeout))
        {
            await SaveAsync("autosave", overwrite: false, ct);
        }

        var refreshEvery = TimeSpan.FromSeconds(Math.Max(10, Host.LockDurationSeconds / 2));
        if (LockHeld && _lockSession is { } ls && now - LastLockRefresh >= refreshEvery)
        {
            try
            {
                await Storage.RefreshLockAsync(ls.Context, FileId, LockId, TimeSpan.FromSeconds(Host.LockDurationSeconds), ct);
                LastLockRefresh = now;
            }
            catch (HostConflictException)
            {
                LockHeld = false;
                LockConflict = true;
                await UpdateModesAsync();
            }
            catch (HostException e)
            {
                _svc.Logger.LogWarning("Lock refresh of {Room} failed: {Error}", Key, e.Message);
            }
        }
    }

    /// <summary>Writes new log entries (incremental batch) or a full snapshot to the object store.</summary>
    public async Task PersistAsync(bool forceSnapshot, CancellationToken ct)
    {
        await _persistGate.WaitAsync(ct);
        try
        {
            List<byte[]> all;
            int persisted;
            bool snapshot;
            string version;
            await _gate.WaitAsync(ct);
            try
            {
                if (_log.Count == 0 || (_persistedCount == _log.Count && !_needSnapshot && !forceSnapshot && _metaVersion == BaseVersion))
                {
                    return;
                }

                all = _log.ToList();
                persisted = _persistedCount;
                snapshot = forceSnapshot || _needSnapshot || Now - _lastSnapshot >= _svc.Options.SnapshotInterval;
                version = BaseVersion;
                _needSnapshot = false;
            }
            finally
            {
                _gate.Release();
            }

            if (_metaVersion != version)
            {
                using var meta = new MemoryStream(JsonSerializer.SerializeToUtf8Bytes(new RoomMeta(Host.Name, FileId, version)));
                await _svc.Store.PutAsync(Prefix + "meta.json", meta, "application/json", ct);
                _metaVersion = version;
            }

            if (snapshot)
            {
                using (var ms = new MemoryStream(YProtocol.EncodeFrames(all)))
                {
                    await _svc.Store.PutAsync(Prefix + "snapshot.bin", ms, "application/octet-stream", ct);
                }

                foreach (var k in await _svc.Store.ListAsync(Prefix + "u-", ct))
                {
                    await _svc.Store.DeleteAsync(k, ct);
                }

                _lastSnapshot = Now;
            }
            else if (all.Count > persisted)
            {
                using var ms = new MemoryStream(YProtocol.EncodeFrames(all.Skip(persisted)));
                await _svc.Store.PutAsync($"{Prefix}u-{++_seq:D10}.bin", ms, "application/octet-stream", ct);
            }

            await WithGateAsync(() => _persistedCount = Math.Max(_persistedCount, all.Count));
        }
        finally
        {
            _persistGate.Release();
        }
    }

    /// <summary>Takes the host lock for the room if possible (first editor).</summary>
    public async Task EnsureLockAsync(EditorSession session, CancellationToken ct)
    {
        if (LockHeld || LockConflict || !session.File.SupportsLocks || !session.Permissions.Edit)
        {
            return;
        }

        try
        {
            await Storage.LockAsync(session.Context, FileId, LockId, TimeSpan.FromSeconds(Host.LockDurationSeconds), ct);
            LockHeld = true;
            _lockSession = session;
            LastLockRefresh = Now;
        }
        catch (HostConflictException)
        {
            LockConflict = true;
            await UpdateModesAsync();
        }
        catch (HostException e)
        {
            _svc.Logger.LogWarning("Lock of {Room} failed: {Error}", Key, e.Message);
        }
    }

    /// <summary>Recomputes every connection's mode and pushes <c>mode-changed</c>; closes connections that lost all access.</summary>
    public async Task UpdateModesAsync()
    {
        EditorSession? newLockOwner = null;
        await WithGateAsync(() =>
        {
            foreach (var c in Connections)
            {
                var mode = ModeOf(c.Session);
                if (mode == c.Mode)
                {
                    continue;
                }

                c.Mode = mode;
                var reason = LockConflict && c.Session.Permissions.Edit ? "locked" : "permissions-changed";
                c.SendEvent(new { type = "mode-changed", mode, reason });
                if (mode == "none")
                {
                    c.SendEvent(new { type = "session-revoked" });
                    c.Close((WebSocketCloseStatus)4403, "access revoked");
                }
                else if (mode == "edit")
                {
                    newLockOwner ??= c.Session;
                }
            }

            if (_lockSession is { Permissions.Edit: false })
            {
                // The lock owner lost edit rights: hand the lock context to another editor.
                _lockSession = EditorConnections().Select(c => c.Session).FirstOrDefault() ?? _lockSession;
            }

            BroadcastCollaborators();
        });
        if (newLockOwner is not null)
        {
            await EnsureLockAsync(newLockOwner, CancellationToken.None);
        }
    }

    /// <summary>
    /// After a restore the host content is authoritative: drop the log, tell clients to <c>reload</c> and close their
    /// sockets (code 4001); clients discard their Y.Doc and reconnect, which starts a fresh room.
    /// </summary>
    public async Task ResetAsync(string version)
    {
        await WithGateAsync(() =>
        {
            _log = [];
            _persistedCount = 0;
            _checkpoint = null;
            _checkpointSaved = true;
            Dirty = false;
            BaseVersion = version;
            _seederId = null;
            _pendingSync.Clear();
            foreach (var c in Connections)
            {
                c.SendEvent(new { type = "reload", version });
                c.Close((WebSocketCloseStatus)4001, "reload");
            }
        });
    }

    /// <summary>Closes every connection of a session.</summary>
    public void Disconnect(EditorSession session, WebSocketCloseStatus status, string reason)
    {
        foreach (var c in Connections.Where(c => c.Session == session))
        {
            c.Close(status, reason);
        }
    }

    // ================================================================ helpers

    /// <summary>Effective mode of a session in this room.</summary>
    public string ModeOf(EditorSession s)
    {
        var mode = s.Permissions.Mode;
        return mode == "edit" && LockConflict ? "view" : mode;
    }

    private void ReassignSeeder()
    {
        _seederId = null;
        if (EditorConnections().FirstOrDefault() is { } next)
        {
            _seederId = next.Id;
            next.SendEvent(new { type = "seed" });
            next.AwaitingFullState = true;
            next.Send(YProtocol.EncodeSync(YProtocol.SyncStep1, YProtocol.EmptyStateVector));
        }

        foreach (var p in _pendingSync.Where(p => p.Id != _seederId))
        {
            p.Send(YProtocol.EncodeSync(YProtocol.SyncStep2, YProtocol.EmptyUpdate));
        }

        _pendingSync.Clear();
    }

    private bool HasConnection(string id) => Connections.Any(c => c.Id == id);

    private List<CollabConnection> EditorConnections() => Connections.Where(c => c.Mode == "edit").OrderBy(c => c.JoinedAt).ToList();

    private void SendLog(CollabConnection conn)
    {
        foreach (var u in _log)
        {
            conn.Send(YProtocol.EncodeSync(YProtocol.SyncStep2, u));
        }
    }

    private void Broadcast(byte[] data, CollabConnection? except)
    {
        foreach (var c in Connections)
        {
            if (c != except)
            {
                c.Send(data);
            }
        }
    }

    private void Broadcast(object evt)
    {
        foreach (var c in Connections)
        {
            c.SendEvent(evt);
        }
    }

    private List<object> Collaborators() => Connections.Select(c => (object)new
    {
        connectionId = c.Id,
        userId = c.Session.User.Id,
        name = c.Session.User.Name,
        avatarUrl = c.Session.User.AvatarUrl,
        mode = c.Mode,
    }).ToList();

    private void BroadcastCollaborators() =>
        Broadcast(new { type = "collaborators-changed", collaborators = Collaborators() });

    private async Task UpdatePeakAsync()
    {
        var count = Connections.Count;
        await using var scope = _svc.Scopes.CreateAsyncScope();
        var db = scope.ServiceProvider.GetRequiredService<BlueOfficeDbContext>();
        if (await db.CollabSessions.FindAsync(SessionRowId) is { } row && row.PeakParticipants < count)
        {
            row.PeakParticipants = count;
            await db.SaveChangesAsync();
        }
    }

    private async Task WithGateAsync(Action action)
    {
        await _gate.WaitAsync();
        try
        {
            action();
        }
        finally
        {
            _gate.Release();
        }
    }

    private sealed record RoomMeta(string Host, string FileId, string BaseVersion);
}
