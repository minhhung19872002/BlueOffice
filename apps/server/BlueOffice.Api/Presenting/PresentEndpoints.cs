using System.Net.WebSockets;
using System.Security.Claims;
using System.Text.Json;
using BlueOffice.Api.Auth;
using BlueOffice.Api.Sessions;

namespace BlueOffice.Api.Presenting;

/// <summary>Body of <c>POST /api/present/remote/claim</c>.</summary>
public sealed record ClaimRequest(string? Token);

/// <summary>
/// Remote control from a phone and online presentation (SLIDE-REMOTE-001). The presenting editor creates a
/// presentation (signed-in user or editor session), then
/// <list type="bullet">
/// <item>asks for a short-lived pairing code shown as a QR code; the phone claims it once and gets its own key;
/// its commands (next / previous / black / end) are relayed to that presenter only;</item>
/// <item>uploads the deck for an online presentation; viewers with the link download it and follow the presenter's
/// slide and animation step (they only receive).</item>
/// </list>
/// Everything stays on this server (no third-party service). Secrets travel in headers (<c>X-Present-Key</c>,
/// <c>X-Present-Token</c>) or, for WebSockets, the query string — never in the path.
/// </summary>
public static class PresentEndpoints
{
    /// <summary>Header of the presenter key.</summary>
    public const string KeyHeader = "X-Present-Key";

    /// <summary>Header of the viewer token (deck download).</summary>
    public const string TokenHeader = "X-Present-Token";

    /// <summary>Remote commands relayed to the presenter.</summary>
    public static readonly IReadOnlySet<string> Actions = new HashSet<string>(StringComparer.Ordinal) { "next", "prev", "first", "last", "black", "white", "end", "start" };

    private const string PptxType = "application/vnd.openxmlformats-officedocument.presentationml.presentation";

    /// <summary>Registers the options and the hub.</summary>
    public static IServiceCollection AddPresent(this IServiceCollection services, IConfiguration config)
    {
        var opts = new PresentOptions();
        config.GetSection("Present").Bind(opts);
        if (opts.PairingTtl <= TimeSpan.Zero || opts.MaxLifetime <= TimeSpan.Zero || opts.MaxMessageBytes <= 0 || opts.MaxDeckBytes <= 0)
        {
            throw new InvalidOperationException("Present: PairingTtl, MaxLifetime, MaxMessageBytes and MaxDeckBytes must be positive.");
        }

        services.AddSingleton(opts);
        services.AddSingleton<PresentHub>();
        services.AddHostedService<PresentPurgeService>();
        return services;
    }

    /// <summary>Maps the routes (404 everywhere when <c>Present:Enabled</c> is false).</summary>
    public static void MapPresent(this IEndpointRouteBuilder app)
    {
        var g = app.MapGroup("/api/present").AllowAnonymous();
        g.AddEndpointFilter(async (ctx, next) =>
            ctx.HttpContext.RequestServices.GetRequiredService<PresentOptions>().Enabled
                ? await next(ctx)
                : Problem(StatusCodes.Status404NotFound, "unavailable", "Remote presenting is turned off on this server."));

        g.MapGet("/config", (PresentOptions o) => Results.Ok(new { enabled = true, pairingSeconds = (int)o.PairingTtl.TotalSeconds }));
        g.MapPost("", Create);
        g.MapDelete("/{id}", (string id, HttpRequest req, PresentHub hub) =>
        {
            if (hub.Presenter(id, req.Headers[KeyHeader]) is not { } p)
            {
                return Results.Unauthorized();
            }

            hub.End(p);
            return Results.NoContent();
        });
        g.MapPost("/{id}/remote", (string id, HttpRequest req, PresentHub hub) =>
        {
            if (hub.Presenter(id, req.Headers[KeyHeader]) is not { } p)
            {
                return Results.Unauthorized();
            }

            var (token, expires) = hub.Pair(p);
            return Results.Ok(new { token, expiresAt = expires, path = "/remote/" + token });
        });
        g.MapDelete("/{id}/remote", (string id, HttpRequest req, PresentHub hub) =>
        {
            if (hub.Presenter(id, req.Headers[KeyHeader]) is not { } p)
            {
                return Results.Unauthorized();
            }

            hub.RevokeRemote(p);
            return Results.NoContent();
        });
        g.MapPost("/{id}/broadcast", BroadcastAsync).DisableAntiforgery();
        g.MapDelete("/{id}/broadcast", (string id, HttpRequest req, PresentHub hub) =>
        {
            if (hub.Presenter(id, req.Headers[KeyHeader]) is not { } p)
            {
                return Results.Unauthorized();
            }

            hub.EndBroadcast(p);
            return Results.NoContent();
        });
        g.MapPost("/remote/claim", (ClaimRequest body, HttpContext ctx, PresentHub hub) =>
        {
            if (!hub.TryRequest("ip:" + ClientAddress(ctx)))
            {
                return Problem(StatusCodes.Status429TooManyRequests, "rateLimited", "Too many attempts; try again in a minute.");
            }

            // Unknown, used and expired codes look the same.
            return hub.Claim(body.Token) is { } key
                ? Results.Ok(new { key })
                : Problem(StatusCodes.Status404NotFound, "invalidToken", "This pairing code is not valid (expired, already used or revoked).");
        });
        g.MapGet("/watch/deck", (HttpContext ctx, PresentHub hub) =>
        {
            if (!hub.TryRequest("ip:" + ClientAddress(ctx)))
            {
                return Problem(StatusCodes.Status429TooManyRequests, "rateLimited", "Too many requests; try again in a minute.");
            }

            if (hub.ByToken(ctx.Request.Headers[TokenHeader], PresentRole.Viewer) is not { } p)
            {
                return Problem(StatusCodes.Status404NotFound, "invalidToken", "This presentation has ended or the link is not valid.");
            }

            byte[]? deck;
            string name;
            lock (p.Gate)
            {
                deck = p.Deck;
                name = p.DeckName;
            }

            if (deck is null)
            {
                return Problem(StatusCodes.Status404NotFound, "invalidToken", "This presentation has ended or the link is not valid.");
            }

            ctx.Response.Headers.CacheControl = "no-store";
            return Results.File(deck, PptxType, name);
        });
        g.Map("/{id}/ws", PresenterSocketAsync);
        g.Map("/remote/ws", RemoteSocketAsync);
        g.Map("/watch/ws", ViewerSocketAsync);
    }

    private static IResult Problem(int status, string code, string detail) =>
        Results.Problem(detail, statusCode: status, extensions: new Dictionary<string, object?> { ["code"] = code });

    private static string ClientAddress(HttpContext ctx) => ctx.Connection.RemoteIpAddress?.ToString() ?? "unknown";

    private static IResult Create(HttpContext ctx, ClaimsPrincipal user, SessionStore sessions, TimeProvider time, PresentHub hub, string? session)
    {
        string owner;
        bool canBroadcast;
        if (session is { Length: > 0 })
        {
            // A hosted file: the editor session's secret, like the dictation and translation proxies.
            if (sessions.Get(session, ctx.Request.Headers[SessionEndpoints.TokenHeader], time.GetUtcNow().UtcDateTime) is not { } s || !s.Permissions.View)
            {
                return Results.Unauthorized();
            }

            owner = "session:" + s.Id;

            // Viewers receive the deck: only when the user may download the file.
            canBroadcast = s.Permissions.Download;
        }
        else if (user.Identity?.IsAuthenticated == true)
        {
            owner = user.PrincipalKey();
            canBroadcast = true;
        }
        else
        {
            return Results.Unauthorized();
        }

        if (!hub.TryRequest("create:" + owner))
        {
            return Problem(StatusCodes.Status429TooManyRequests, "rateLimited", "Too many presentations started; try again in a minute.");
        }

        if (hub.Create(owner, canBroadcast) is not { } created)
        {
            return Problem(StatusCodes.Status429TooManyRequests, "tooMany", $"At most {hub.Options.MaxPerOwner} presentations at once.");
        }

        return Results.Ok(new { id = created.P.Id, key = created.Key, expiresAt = created.P.ExpiresAt, canBroadcast });
    }

    private static async Task<IResult> BroadcastAsync(string id, string? name, HttpRequest req, PresentHub hub, CancellationToken ct)
    {
        if (hub.Presenter(id, req.Headers[KeyHeader]) is not { } p)
        {
            return Results.Unauthorized();
        }

        if (!p.CanBroadcast)
        {
            return Problem(StatusCodes.Status403Forbidden, "forbidden", "Downloading this file is not allowed, so it cannot be presented online.");
        }

        var max = hub.Options.MaxDeckBytes;
        if (req.ContentLength > max)
        {
            return Problem(StatusCodes.Status413PayloadTooLarge, "tooLarge", $"At most {max} bytes.");
        }

        using var buf = new MemoryStream();
        var chunk = new byte[64 * 1024];
        int n;
        while ((n = await req.Body.ReadAsync(chunk, ct)) > 0)
        {
            if (buf.Length + n > max)
            {
                return Problem(StatusCodes.Status413PayloadTooLarge, "tooLarge", $"At most {max} bytes.");
            }

            buf.Write(chunk, 0, n);
        }

        // ECMA-376 Part 2 (OPC): a package is a ZIP archive ("PK" local file header).
        var deck = buf.ToArray();
        if (deck.Length < 4 || deck[0] != 'P' || deck[1] != 'K')
        {
            return Problem(StatusCodes.Status400BadRequest, "invalid", "The body must be a .pptx package.");
        }

        var file = Path.GetFileName(name ?? "");
        var token = hub.Broadcast(p, deck, file.Length is > 0 and <= 200 ? file : "deck.pptx");
        return Results.Ok(new { token = token.Length > 0 ? token : null, refreshed = token.Length == 0 });
    }

    private static async Task PresenterSocketAsync(HttpContext ctx, string id, PresentHub hub)
    {
        if (!ctx.WebSockets.IsWebSocketRequest)
        {
            ctx.Response.StatusCode = StatusCodes.Status400BadRequest;
            return;
        }

        if (hub.Presenter(id, ctx.Request.Query["key"]) is not { } p)
        {
            ctx.Response.StatusCode = StatusCodes.Status401Unauthorized;
            return;
        }

        using var socket = await ctx.WebSockets.AcceptWebSocketAsync();
        var peer = new PresentPeer(socket);
        PresentPeer? old;
        lock (p.Gate)
        {
            // One presenter: a reconnecting presenter replaces its previous connection.
            old = p.Presenter;
            p.Presenter = peer;
            p.PresenterSeen = hub.Now;
        }

        old?.Close(WebSocketCloseStatus.PolicyViolation, "replaced");
        PresentHub.NotifyPresenter(p);
        PresentHub.NotifyRemote(p);
        await RunAsync(socket, peer, hub.Options.MaxMessageBytes, data =>
        {
            switch (TypeOf(data))
            {
                case "remote":
                    PresentPeer? remote;
                    lock (p.Gate)
                    {
                        p.LastRemote = data;
                        remote = p.Remote;
                    }

                    remote?.Send(data);
                    break;
                case "show":
                    List<PresentPeer> viewers;
                    lock (p.Gate)
                    {
                        if (p.ViewerHash is null)
                        {
                            break;
                        }

                        p.LastShow = data;
                        viewers = [.. p.Viewers];
                    }

                    foreach (var v in viewers)
                    {
                        v.Send(data);
                    }

                    break;
                default:
                    break;
            }
        }, ctx.RequestAborted);
        var mine = false;
        lock (p.Gate)
        {
            if (p.Presenter == peer)
            {
                p.Presenter = null;
                p.PresenterSeen = hub.Now;
                mine = true;
            }
        }

        if (mine)
        {
            PresentHub.NotifyRemote(p);
        }
    }

    private static async Task RemoteSocketAsync(HttpContext ctx, PresentHub hub)
    {
        if (!ctx.WebSockets.IsWebSocketRequest)
        {
            ctx.Response.StatusCode = StatusCodes.Status400BadRequest;
            return;
        }

        if (hub.Remote(ctx.Request.Query["key"]) is not { } p)
        {
            ctx.Response.StatusCode = StatusCodes.Status401Unauthorized;
            return;
        }

        using var socket = await ctx.WebSockets.AcceptWebSocketAsync();
        var peer = new PresentPeer(socket);
        PresentPeer? old;
        byte[]? last;
        lock (p.Gate)
        {
            old = p.Remote;
            p.Remote = peer;
            last = p.LastRemote;
        }

        old?.Close(WebSocketCloseStatus.PolicyViolation, "replaced");
        PresentHub.NotifyRemote(p);
        if (last is not null)
        {
            peer.Send(last);
        }

        PresentHub.NotifyPresenter(p);
        await RunAsync(socket, peer, 4096, data =>
        {
            string? action = null;
            try
            {
                using var doc = JsonDocument.Parse(data);
                if (doc.RootElement.ValueKind == JsonValueKind.Object
                    && doc.RootElement.TryGetProperty("type", out var t) && t.ValueKind == JsonValueKind.String && t.GetString() == "command"
                    && doc.RootElement.TryGetProperty("action", out var a) && a.ValueKind == JsonValueKind.String)
                {
                    action = a.GetString();
                }
            }
            catch (JsonException)
            {
                // Ignored below.
            }

            if (action is null || !Actions.Contains(action))
            {
                peer.SendEvent(new { type = "error", code = "invalid" });
                return;
            }

            if (!hub.TryCommand(p))
            {
                peer.SendEvent(new { type = "error", code = "rateLimited" });
                return;
            }

            PresentPeer? presenter;
            lock (p.Gate)
            {
                presenter = p.Remote == peer ? p.Presenter : null;
            }

            if (presenter is null)
            {
                peer.SendEvent(new { type = "error", code = "presenterOffline" });
                return;
            }

            // Re-serialised: only the allowed action reaches the presenter.
            presenter.SendEvent(new { type = "command", action });
        }, ctx.RequestAborted);
        var mine = false;
        lock (p.Gate)
        {
            if (p.Remote == peer)
            {
                p.Remote = null;
                mine = true;
            }
        }

        if (mine)
        {
            PresentHub.NotifyPresenter(p);
        }
    }

    private static async Task ViewerSocketAsync(HttpContext ctx, PresentHub hub)
    {
        if (!ctx.WebSockets.IsWebSocketRequest)
        {
            ctx.Response.StatusCode = StatusCodes.Status400BadRequest;
            return;
        }

        if (!hub.TryRequest("ip:" + ClientAddress(ctx)))
        {
            ctx.Response.StatusCode = StatusCodes.Status429TooManyRequests;
            return;
        }

        if (hub.ByToken(ctx.Request.Query["token"], PresentRole.Viewer) is not { } p)
        {
            ctx.Response.StatusCode = StatusCodes.Status401Unauthorized;
            return;
        }

        lock (p.Gate)
        {
            if (p.Viewers.Count >= hub.Options.MaxViewers)
            {
                ctx.Response.StatusCode = StatusCodes.Status503ServiceUnavailable;
                return;
            }
        }

        using var socket = await ctx.WebSockets.AcceptWebSocketAsync();
        var peer = new PresentPeer(socket);
        byte[]? last;
        lock (p.Gate)
        {
            p.Viewers.Add(peer);
            last = p.LastShow;
        }

        if (last is not null)
        {
            peer.Send(last);
        }

        PresentHub.NotifyPresenter(p);

        // Viewers only receive: whatever they send is dropped.
        await RunAsync(socket, peer, 4096, _ => { }, ctx.RequestAborted);
        bool removed;
        lock (p.Gate)
        {
            removed = p.Viewers.Remove(peer);
        }

        if (removed)
        {
            PresentHub.NotifyPresenter(p);
        }
    }

    /// <summary>JSON <c>type</c> of a message, or <c>null</c>.</summary>
    private static string? TypeOf(byte[] data)
    {
        try
        {
            // Only the top-level "type" property is looked at; the rest is relayed as it is.
            using var doc = JsonDocument.Parse(data);
            return doc.RootElement.ValueKind == JsonValueKind.Object
                && doc.RootElement.TryGetProperty("type", out var t) && t.ValueKind == JsonValueKind.String ? t.GetString() : null;
        }
        catch (JsonException)
        {
            return null;
        }
    }

    /// <summary>Runs the send loop and the receive loop of <paramref name="peer"/> until the socket closes.</summary>
    private static async Task RunAsync(WebSocket socket, PresentPeer peer, int maxBytes, Action<byte[]> onMessage, CancellationToken ct)
    {
        var send = peer.RunSendLoopAsync(ct);
        var buffer = new byte[16 * 1024];
        try
        {
            while (socket.State == WebSocketState.Open)
            {
                using var ms = new MemoryStream();
                WebSocketReceiveResult r;
                var tooBig = false;
                do
                {
                    r = await socket.ReceiveAsync(buffer, ct);
                    if (ms.Length + r.Count > maxBytes)
                    {
                        tooBig = true;
                        break;
                    }

                    ms.Write(buffer, 0, r.Count);
                }
                while (!r.EndOfMessage);

                if (tooBig)
                {
                    peer.Close(WebSocketCloseStatus.MessageTooBig, "message too big");
                    break;
                }

                if (r.MessageType == WebSocketMessageType.Close)
                {
                    peer.Close(WebSocketCloseStatus.NormalClosure, "");
                    break;
                }

                if (r.MessageType == WebSocketMessageType.Text)
                {
                    onMessage(ms.ToArray());
                }
            }
        }
        catch (Exception e) when (e is WebSocketException or OperationCanceledException or IOException)
        {
            // Closed.
        }

        peer.Close(WebSocketCloseStatus.NormalClosure, "");
        await send;
    }
}
