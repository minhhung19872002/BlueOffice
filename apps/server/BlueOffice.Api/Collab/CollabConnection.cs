using System.Net.WebSockets;
using System.Text.Json;
using System.Threading.Channels;
using BlueOffice.Api.Sessions;

namespace BlueOffice.Api.Collab;

/// <summary>One WebSocket participant. Outgoing frames go through a bounded queue drained by a single send loop.</summary>
public sealed class CollabConnection(string id, EditorSession session, WebSocket socket, DateTime joinedAt)
{
    /// <summary>JSON options for control events.</summary>
    public static readonly JsonSerializerOptions Json = new(JsonSerializerDefaults.Web);

    private readonly Channel<(byte[] Data, WebSocketMessageType Type)> _outbox =
        Channel.CreateBounded<(byte[], WebSocketMessageType)>(new BoundedChannelOptions(4096) { SingleReader = true });

    private WebSocketCloseStatus _closeStatus = WebSocketCloseStatus.NormalClosure;
    private string _closeReason = "";

    /// <summary>Connection id.</summary>
    public string Id { get; } = id;

    /// <summary>Editor session (user, permissions, host context).</summary>
    public EditorSession Session { get; } = session;

    /// <summary>Join time.</summary>
    public DateTime JoinedAt { get; } = joinedAt;

    /// <summary>Awareness client ids announced by this connection → last clock.</summary>
    public Dictionary<ulong, ulong> AwarenessClients { get; } = [];

    /// <summary>Mode last announced to the client.</summary>
    public string Mode { get; set; } = "view";

    /// <summary>The server sent this client a step 1 with an empty state vector and waits for its full state.</summary>
    public bool AwaitingFullState { get; set; }

    /// <summary>Queues a binary y-protocols frame.</summary>
    public void Send(byte[] data) => Enqueue(data, WebSocketMessageType.Binary);

    /// <summary>Queues a JSON control event (text frame).</summary>
    public void SendEvent(object evt) => Enqueue(JsonSerializer.SerializeToUtf8Bytes(evt, Json), WebSocketMessageType.Text);

    private void Enqueue(byte[] data, WebSocketMessageType type)
    {
        if (!_outbox.Writer.TryWrite((data, type)))
        {
            // Slow consumer: drop the connection rather than buffering without bound.
            Close(WebSocketCloseStatus.PolicyViolation, "slow consumer");
        }
    }

    /// <summary>Flushes queued frames then closes with <paramref name="status"/>.</summary>
    public void Close(WebSocketCloseStatus status, string reason)
    {
        _closeStatus = status;
        _closeReason = reason;
        if (_outbox.Writer.TryComplete())
        {
            // Abort if the peer never answers the close handshake.
            _ = Task.Delay(TimeSpan.FromSeconds(5)).ContinueWith(_ => socket.Abort(), TaskScheduler.Default);
        }
    }

    /// <summary>Drains the outbox until <see cref="Close"/> is called or the socket fails.</summary>
    public async Task RunSendLoopAsync(CancellationToken ct)
    {
        try
        {
            await foreach (var (data, type) in _outbox.Reader.ReadAllAsync(ct))
            {
                await socket.SendAsync(data, type, endOfMessage: true, ct);
            }

            if (socket.State is WebSocketState.Open or WebSocketState.CloseReceived)
            {
                await socket.CloseOutputAsync(_closeStatus, _closeReason, ct);
            }
        }
        catch (Exception e) when (e is WebSocketException or OperationCanceledException or ObjectDisposedException)
        {
            // Connection gone; the receive loop handles cleanup.
        }
    }
}
