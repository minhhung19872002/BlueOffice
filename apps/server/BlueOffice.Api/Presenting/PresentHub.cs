using System.Collections.Concurrent;
using System.Net.WebSockets;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Threading.Channels;

namespace BlueOffice.Api.Presenting;

/// <summary>Options of remote control and online presentation (<c>Present</c> section; docs/deploy/security.md).</summary>
public sealed class PresentOptions
{
    /// <summary>Off: every <c>/api/present</c> route answers 404.</summary>
    public bool Enabled { get; set; } = true;

    /// <summary>How long a pairing code (QR) can be used by a phone.</summary>
    public TimeSpan PairingTtl { get; set; } = TimeSpan.FromMinutes(5);

    /// <summary>Longest life of a presentation (remote control and online link end with it).</summary>
    public TimeSpan MaxLifetime { get; set; } = TimeSpan.FromHours(12);

    /// <summary>A presentation whose presenter stayed disconnected this long ends.</summary>
    public TimeSpan IdleTimeout { get; set; } = TimeSpan.FromMinutes(30);

    /// <summary>Largest deck of an online presentation.</summary>
    public long MaxDeckBytes { get; set; } = 100L * 1024 * 1024;

    /// <summary>Largest WebSocket message (the remote's state carries two slide thumbnails).</summary>
    public int MaxMessageBytes { get; set; } = 1024 * 1024;

    /// <summary>Viewers of one online presentation.</summary>
    public int MaxViewers { get; set; } = 200;

    /// <summary>Presentations open at once per user / editor session.</summary>
    public int MaxPerOwner { get; set; } = 5;

    /// <summary>Remote commands per <see cref="CommandWindow"/>.</summary>
    public int CommandLimit { get; set; } = 40;

    /// <summary>Window of <see cref="CommandLimit"/>.</summary>
    public TimeSpan CommandWindow { get; set; } = TimeSpan.FromSeconds(10);

    /// <summary>Token uses (pairing, deck download, viewer join, creation) per client address per <see cref="RequestWindow"/>.</summary>
    public int RequestLimit { get; set; } = 60;

    /// <summary>Window of <see cref="RequestLimit"/>.</summary>
    public TimeSpan RequestWindow { get; set; } = TimeSpan.FromMinutes(1);
}

/// <summary>Ends expired and idle presentations once a minute (lookups also check expiry).</summary>
public sealed class PresentPurgeService(PresentHub hub) : BackgroundService
{
    /// <inheritdoc />
    protected override async Task ExecuteAsync(CancellationToken stoppingToken)
    {
        using var timer = new PeriodicTimer(TimeSpan.FromMinutes(1));
        try
        {
            while (await timer.WaitForNextTickAsync(stoppingToken))
            {
                hub.Purge();
            }
        }
        catch (OperationCanceledException)
        {
            // Stopping.
        }
    }
}

/// <summary>Who holds a token.</summary>
public enum PresentRole
{
    /// <summary>The presenting editor (key returned at creation).</summary>
    Presenter,

    /// <summary>The phone remote (key returned when the pairing code is claimed).</summary>
    Remote,

    /// <summary>A viewer of the online presentation (link token).</summary>
    Viewer,
}

/// <summary>One WebSocket peer; frames go through a bounded queue drained by one send loop.</summary>
public sealed class PresentPeer(WebSocket socket)
{
    private readonly Channel<byte[]> _outbox = Channel.CreateBounded<byte[]>(new BoundedChannelOptions(256) { SingleReader = true });
    private WebSocketCloseStatus _status = WebSocketCloseStatus.NormalClosure;
    private string _reason = "";
    private int _closing;

    /// <summary>Queues a text frame (dropped peer when it does not keep up).</summary>
    public void Send(byte[] json)
    {
        if (!_outbox.Writer.TryWrite(json) && Volatile.Read(ref _closing) == 0)
        {
            Close(WebSocketCloseStatus.PolicyViolation, "slow consumer");
        }
    }

    /// <summary>Queues a JSON event.</summary>
    public void SendEvent(object evt) => Send(JsonSerializer.SerializeToUtf8Bytes(evt, PresentHub.Json));

    /// <summary>Flushes queued frames then closes; the first call decides the status.</summary>
    public void Close(WebSocketCloseStatus status, string reason)
    {
        if (Interlocked.Exchange(ref _closing, 1) != 0)
        {
            return;
        }

        _status = status;
        _reason = reason;
        if (_outbox.Writer.TryComplete())
        {
            _ = Task.Delay(TimeSpan.FromSeconds(5)).ContinueWith(_ => socket.Abort(), TaskScheduler.Default);
        }
    }

    /// <summary>Drains the outbox until closed.</summary>
    public async Task RunSendLoopAsync(CancellationToken ct)
    {
        try
        {
            await foreach (var data in _outbox.Reader.ReadAllAsync(ct))
            {
                await socket.SendAsync(data, WebSocketMessageType.Text, endOfMessage: true, ct);
            }

            if (socket.State is WebSocketState.Open or WebSocketState.CloseReceived)
            {
                await socket.CloseOutputAsync(_status, _reason, ct);
            }
        }
        catch (Exception e) when (e is WebSocketException or OperationCanceledException or ObjectDisposedException)
        {
            // Gone; the receive loop cleans up.
        }
    }
}

/// <summary>A running presentation: one presenter, at most one paired phone, any number of viewers (up to
/// <see cref="PresentOptions.MaxViewers"/>). Everything is in memory on the instance that created it.</summary>
public sealed class Presentation
{
    /// <summary>Guards the mutable state below.</summary>
    public object Gate { get; } = new();

    /// <summary>Public id (URLs of the presenter's calls).</summary>
    public required string Id { get; init; }

    /// <summary>User / session that created it.</summary>
    public required string Owner { get; init; }

    /// <summary>The owner may hand the deck to viewers (a hosted session needs the download permission).</summary>
    public required bool CanBroadcast { get; init; }

    /// <summary>Creation time.</summary>
    public required DateTimeOffset CreatedAt { get; init; }

    /// <summary>Hard end.</summary>
    public required DateTimeOffset ExpiresAt { get; init; }

    /// <summary>Last time a presenter was connected (idle timeout).</summary>
    public DateTimeOffset PresenterSeen { get; set; }

    /// <summary>Hash of the presenter key.</summary>
    public string? PresenterHash { get; set; }

    /// <summary>Hash of the pairing code and its end.</summary>
    public string? PairingHash { get; set; }

    /// <summary>End of the pairing code.</summary>
    public DateTimeOffset PairingExpiresAt { get; set; }

    /// <summary>Hash of the paired phone's key.</summary>
    public string? RemoteHash { get; set; }

    /// <summary>Hash of the viewer link token.</summary>
    public string? ViewerHash { get; set; }

    /// <summary>Deck of the online presentation.</summary>
    public byte[]? Deck { get; set; }

    /// <summary>File name of <see cref="Deck"/>.</summary>
    public string DeckName { get; set; } = "deck.pptx";

    /// <summary>Last state for the phone (replayed when it connects).</summary>
    public byte[]? LastRemote { get; set; }

    /// <summary>Last show state for viewers (replayed when they join).</summary>
    public byte[]? LastShow { get; set; }

    /// <summary>Connected presenter.</summary>
    public PresentPeer? Presenter { get; set; }

    /// <summary>Connected phone.</summary>
    public PresentPeer? Remote { get; set; }

    /// <summary>Connected viewers.</summary>
    public List<PresentPeer> Viewers { get; } = [];

    /// <summary>Remote commands in the current window.</summary>
    public (DateTimeOffset Start, int Count) Commands { get; set; }
}

/// <summary>In-memory registry of presentations. Tokens are 128-bit random values; only their SHA-256 hashes are
/// kept as keys, so lookups do not compare secrets character by character.</summary>
public sealed class PresentHub(PresentOptions options, TimeProvider time)
{
    /// <summary>JSON options of the events.</summary>
    public static readonly JsonSerializerOptions Json = new(JsonSerializerDefaults.Web);

    private readonly ConcurrentDictionary<string, Presentation> _byId = new();
    private readonly ConcurrentDictionary<string, (Presentation P, PresentRole Role)> _tokens = new();
    private readonly ConcurrentDictionary<string, (DateTimeOffset Start, int Count)> _requests = new();

    /// <summary>Options.</summary>
    public PresentOptions Options => options;

    /// <summary>Now.</summary>
    public DateTimeOffset Now => time.GetUtcNow();

    /// <summary>A new 128-bit random token (base64url, 22 characters).</summary>
    public static string NewToken() => Base64Url(RandomNumberGenerator.GetBytes(16));

    private static string Base64Url(byte[] b) => Convert.ToBase64String(b).TrimEnd('=').Replace('+', '-').Replace('/', '_');

    private static string Hash(string token) => Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(token)));

    /// <summary>Presentations open now.</summary>
    public int Count => _byId.Count;

    /// <summary>Counts a request of <paramref name="key"/> (client address); <c>false</c> when over the limit.</summary>
    public bool TryRequest(string key)
    {
        if (options.RequestLimit <= 0)
        {
            return true;
        }

        var now = Now;
        if (_requests.Count > 10_000)
        {
            foreach (var (k, w) in _requests)
            {
                if (now - w.Start >= options.RequestWindow)
                {
                    _requests.TryRemove(k, out _);
                }
            }
        }

        var next = _requests.AddOrUpdate(key, _ => (now, 1), (_, w) => now - w.Start >= options.RequestWindow ? (now, 1) : (w.Start, w.Count + 1));
        return next.Count <= options.RequestLimit;
    }

    /// <summary>Counts a remote command; <c>false</c> when over the limit.</summary>
    public bool TryCommand(Presentation p)
    {
        var now = Now;
        lock (p.Gate)
        {
            var w = p.Commands;
            w = now - w.Start >= options.CommandWindow ? (now, 1) : (w.Start, w.Count + 1);
            p.Commands = w;
            return options.CommandLimit <= 0 || w.Count <= options.CommandLimit;
        }
    }

    /// <summary>Creates a presentation; returns it with the presenter key, or <c>null</c> when the owner has too many.</summary>
    public (Presentation P, string Key)? Create(string owner, bool canBroadcast)
    {
        Purge();
        if (_byId.Values.Count(x => x.Owner == owner) >= options.MaxPerOwner)
        {
            return null;
        }

        var now = Now;
        var key = NewToken();
        var p = new Presentation
        {
            Id = NewToken(),
            Owner = owner,
            CanBroadcast = canBroadcast,
            CreatedAt = now,
            ExpiresAt = now + options.MaxLifetime,
            PresenterSeen = now,
            PresenterHash = Hash(key),
        };
        _byId[p.Id] = p;
        _tokens[p.PresenterHash] = (p, PresentRole.Presenter);
        return (p, key);
    }

    /// <summary>The presentation of <paramref name="id"/> when <paramref name="key"/> is its presenter key.</summary>
    public Presentation? Presenter(string? id, string? key)
    {
        if (id is null || key is null || !_byId.TryGetValue(id, out var p) || !Live(p))
        {
            return null;
        }

        var h = Hash(key);
        return p.PresenterHash is { } ph && CryptographicOperations.FixedTimeEquals(Encoding.ASCII.GetBytes(ph), Encoding.ASCII.GetBytes(h)) ? p : null;
    }

    /// <summary>The live presentation a token of <paramref name="role"/> belongs to.</summary>
    public Presentation? ByToken(string? token, PresentRole role)
    {
        if (string.IsNullOrEmpty(token) || token.Length > 64 || !_tokens.TryGetValue(Hash(token), out var e) || e.Role != role || !Live(e.P))
        {
            return null;
        }

        return e.P;
    }

    private bool Live(Presentation p)
    {
        var now = Now;
        if (now >= p.ExpiresAt || (p.Presenter is null && now - p.PresenterSeen >= options.IdleTimeout))
        {
            End(p, "expired");
            return false;
        }

        return true;
    }

    /// <summary>A new pairing code for a phone; the previous phone (and code) are revoked.</summary>
    public (string Token, DateTimeOffset ExpiresAt) Pair(Presentation p)
    {
        RevokeRemote(p);
        var token = NewToken();
        var hash = Hash(token);
        lock (p.Gate)
        {
            p.PairingHash = hash;
            p.PairingExpiresAt = Now + options.PairingTtl;
        }

        _tokens[hash] = (p, PresentRole.Remote);
        return (token, p.PairingExpiresAt);
    }

    /// <summary>Exchanges a pairing code (once, before it expires) for the phone's key.</summary>
    public string? Claim(string? pairing)
    {
        if (string.IsNullOrEmpty(pairing) || pairing.Length > 64)
        {
            return null;
        }

        var hash = Hash(pairing);
        if (!_tokens.TryGetValue(hash, out var e) || !Live(e.P))
        {
            return null;
        }

        var p = e.P;
        string key;
        lock (p.Gate)
        {
            if (p.PairingHash != hash)
            {
                // A phone key, not a pairing code.
                return null;
            }

            _tokens.TryRemove(hash, out _);
            p.PairingHash = null;
            if (Now >= p.PairingExpiresAt)
            {
                return null;
            }

            key = NewToken();
            p.RemoteHash = Hash(key);
        }

        _tokens[p.RemoteHash] = (p, PresentRole.Remote);
        return key;
    }

    /// <summary>The presentation of a paired phone's key (pairing codes do not connect).</summary>
    public Presentation? Remote(string? key)
    {
        var p = ByToken(key, PresentRole.Remote);
        if (p is null || key is null)
        {
            return null;
        }

        lock (p.Gate)
        {
            return p.RemoteHash == Hash(key) ? p : null;
        }
    }

    /// <summary>Revokes the pairing code and the paired phone (closed with 4401).</summary>
    public void RevokeRemote(Presentation p)
    {
        PresentPeer? remote;
        lock (p.Gate)
        {
            if (p.PairingHash is { } ph)
            {
                _tokens.TryRemove(ph, out _);
            }

            if (p.RemoteHash is { } rh)
            {
                _tokens.TryRemove(rh, out _);
            }

            p.PairingHash = null;
            p.RemoteHash = null;
            remote = p.Remote;
            p.Remote = null;
        }

        if (remote is not null)
        {
            remote.SendEvent(new { type = "revoked" });
            remote.Close((WebSocketCloseStatus)4401, "revoked");
            NotifyPresenter(p);
        }
    }

    /// <summary>Starts (or refreshes) the online presentation with <paramref name="deck"/>; returns the viewer token
    /// (the same one on a refresh).</summary>
    public string Broadcast(Presentation p, byte[] deck, string name)
    {
        string? token = null;
        List<PresentPeer> viewers;
        lock (p.Gate)
        {
            p.Deck = deck;
            p.DeckName = name;
            viewers = [.. p.Viewers];
            if (p.ViewerHash is null)
            {
                token = NewToken();
                p.ViewerHash = Hash(token);
            }
        }

        if (token is not null)
        {
            _tokens[Hash(token)] = (p, PresentRole.Viewer);
            return token;
        }

        // A refreshed deck: viewers open it again (same link).
        foreach (var v in viewers)
        {
            v.SendEvent(new { type = "reload" });
        }

        return "";
    }

    /// <summary>Ends the online presentation: viewers are told and closed, the link stops working.</summary>
    public void EndBroadcast(Presentation p)
    {
        List<PresentPeer> viewers;
        lock (p.Gate)
        {
            if (p.ViewerHash is { } vh)
            {
                _tokens.TryRemove(vh, out _);
            }

            p.ViewerHash = null;
            p.Deck = null;
            p.LastShow = null;
            viewers = [.. p.Viewers];
            p.Viewers.Clear();
        }

        foreach (var v in viewers)
        {
            v.SendEvent(new { type = "ended" });
            v.Close((WebSocketCloseStatus)4410, "ended");
        }

        NotifyPresenter(p);
    }

    /// <summary>Ends the presentation: every peer is told and closed, every token stops working.</summary>
    public void End(Presentation p, string reason = "ended")
    {
        if (!_byId.TryRemove(p.Id, out _))
        {
            return;
        }

        List<PresentPeer> peers;
        lock (p.Gate)
        {
            foreach (var h in new[] { p.PresenterHash, p.PairingHash, p.RemoteHash, p.ViewerHash })
            {
                if (h is not null)
                {
                    _tokens.TryRemove(h, out _);
                }
            }

            p.PresenterHash = p.PairingHash = p.RemoteHash = p.ViewerHash = null;
            p.Deck = null;
            peers = [.. p.Viewers];
            if (p.Remote is { } r)
            {
                peers.Add(r);
            }

            if (p.Presenter is { } pr)
            {
                peers.Add(pr);
            }

            p.Viewers.Clear();
            p.Remote = null;
            p.Presenter = null;
        }

        foreach (var peer in peers)
        {
            peer.SendEvent(new { type = "ended", reason });
            peer.Close((WebSocketCloseStatus)4410, reason);
        }
    }

    /// <summary>Ends expired and idle presentations.</summary>
    public void Purge()
    {
        foreach (var p in _byId.Values)
        {
            _ = Live(p);
        }
    }

    /// <summary>Tells the presenter who is connected.</summary>
    public static void NotifyPresenter(Presentation p)
    {
        PresentPeer? presenter;
        object evt;
        lock (p.Gate)
        {
            presenter = p.Presenter;
            evt = new { type = "peers", remote = p.Remote is not null, paired = p.RemoteHash is not null, viewers = p.Viewers.Count };
        }

        presenter?.SendEvent(evt);
    }

    /// <summary>Tells the phone whether the presenter is connected.</summary>
    public static void NotifyRemote(Presentation p)
    {
        PresentPeer? remote;
        bool presenter;
        lock (p.Gate)
        {
            remote = p.Remote;
            presenter = p.Presenter is not null;
        }

        remote?.SendEvent(new { type = "peers", presenter });
    }
}
