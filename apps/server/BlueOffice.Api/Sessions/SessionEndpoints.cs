using System.Net.WebSockets;
using System.Security.Claims;
using System.Text;
using BlueOffice.Api.Audit;
using BlueOffice.Api.Auth;
using BlueOffice.Api.Collab;
using BlueOffice.Api.Common;
using BlueOffice.Domain;
using BlueOffice.Infrastructure.Hosts;
using BlueOffice.Infrastructure.Storage;

namespace BlueOffice.Api.Sessions;

/// <summary>Client-reported action (print / export / download) inside the editor.</summary>
public sealed record SessionAuditRequest(AuditAction Action, string? Detail);

/// <summary>Body of /refresh.</summary>
public sealed record RefreshRequest(string? Token);

/// <summary>Body of /rename.</summary>
public sealed record RenameRequest(string Name);

/// <summary>Host → BlueOffice notification (e.g. permissions changed).</summary>
public sealed record HostNotification(string Event, string FileId, string? UserId);

/// <summary>Editor session API (Phase 6A), collaboration WebSocket and WOPI discovery.</summary>
public static class SessionEndpoints
{
    /// <summary>Header carrying the session secret.</summary>
    public const string TokenHeader = "X-Session-Token";

    /// <summary>Maps the routes.</summary>
    public static void MapSessionEndpoints(this IEndpointRouteBuilder app)
    {
        // Editor entry point used by hosts (iframe URL). Returns the session JSON; the web editor shell reads the same data.
        app.MapGet("/edit", (HttpRequest req, SessionService sessions, CancellationToken ct) => CreateFromQueryAsync(req, null, sessions, ct))
            .AllowAnonymous();
        app.MapPost("/edit", async (HttpRequest req, SessionService sessions, CancellationToken ct) =>
        {
            // WOPI hosts POST access_token as a form field to the action URL ([MS-WOPI] §3.1.5.1.1.2).
            var form = req.HasFormContentType ? await req.ReadFormAsync(ct) : null;
            return await CreateFromQueryAsync(req, form?["access_token"], sessions, ct);
        }).AllowAnonymous().DisableAntiforgery();

        var api = app.MapGroup("/api/sessions").AllowAnonymous();
        api.MapPost("", async (CreateSessionRequest body, ClaimsPrincipal p, SessionService sessions, CancellationToken ct) =>
        {
            SessionInfo info;
            if (!string.IsNullOrEmpty(body.WopiSrc))
            {
                info = await sessions.CreateWopiSessionAsync(body.WopiSrc, body.Token, ct);
            }
            else if (!string.IsNullOrEmpty(body.Host) && !string.Equals(body.Host, "local", StringComparison.OrdinalIgnoreCase))
            {
                info = await sessions.CreateHostSessionAsync(body.Host, body.FileId ?? throw ApiException.BadRequest("fileId required."), body.Token, ct);
            }
            else
            {
                if (p.Identity?.IsAuthenticated != true)
                {
                    throw new ApiException(StatusCodes.Status401Unauthorized, "Sign in to open BlueOffice files.");
                }

                info = await sessions.CreateLocalSessionAsync(p, body.FileId ?? throw ApiException.BadRequest("fileId required."), ct);
            }

            return Results.Created($"/api/sessions/{info.SessionId}", info);
        });
        api.MapGet("/{id}", (string id, HttpRequest req, SessionStore store, SessionService sessions, TimeProvider time) =>
            Results.Ok(sessions.Info(Require(id, req, store, time), includeToken: false)));
        api.MapDelete("/{id}", (string id, HttpRequest req, SessionStore store, RoomManager rooms, TimeProvider time) =>
        {
            var s = Require(id, req, store, time);
            store.Remove(s.Id);
            rooms.Find(s.RoomKey)?.Disconnect(s, WebSocketCloseStatus.NormalClosure, "session closed");
            return Results.NoContent();
        });
        api.MapGet("/{id}/contents", async (string id, HttpRequest req, SessionStore store, TimeProvider time, CancellationToken ct) =>
        {
            var s = Require(id, req, store, time);
            var content = await s.Storage.GetContentsAsync(s.Context, s.FileId, ct);
            req.HttpContext.Response.Headers["X-Version"] = content.Version ?? s.File.Version;
            return Results.File(content.Content, FileContentType(s.File.Name), s.File.Name);
        });
        api.MapPost("/{id}/checkpoint", CheckpointAsync).DisableAntiforgery();
        api.MapPost("/{id}/refresh", async (string id, RefreshRequest? body, HttpRequest req, SessionStore store, SessionService sessions, TimeProvider time, CancellationToken ct) =>
            Results.Ok(await sessions.RefreshAsync(Require(id, req, store, time), body?.Token, ct)));
        api.MapPost("/{id}/audit", async (string id, SessionAuditRequest body, HttpRequest req, SessionStore store, AuditService audit, TimeProvider time, CancellationToken ct) =>
        {
            var s = Require(id, req, store, time);
            if (body.Action is not (AuditAction.Print or AuditAction.Export or AuditAction.Download))
            {
                throw ApiException.BadRequest("Only print, export and download can be reported.");
            }

            if (body.Action is AuditAction.Export or AuditAction.Download && !s.Permissions.Download)
            {
                throw ApiException.Forbidden("Download is not allowed.");
            }

            await audit.RecordAsync(body.Action, ResourceKind.File, s.FileId, s.User.Id, s.User.Name, body.Detail, s.Host, ct);
            return Results.NoContent();
        });
        api.MapGet("/{id}/versions", async (string id, HttpRequest req, SessionStore store, TimeProvider time, CancellationToken ct) =>
        {
            var s = Require(id, req, store, time);
            return Results.Ok(new { versions = await s.Storage.GetVersionsAsync(s.Context, s.FileId, ct) });
        });
        api.MapGet("/{id}/versions/{version}/contents", async (string id, string version, HttpRequest req, SessionStore store, TimeProvider time, CancellationToken ct) =>
        {
            // An older version (compare, preview before restore): read-only, any session that can view the file.
            var s = Require(id, req, store, time);
            var content = await s.Storage.GetVersionContentsAsync(s.Context, s.FileId, version, ct);
            req.HttpContext.Response.Headers["X-Version"] = content.Version ?? version;
            return Results.File(content.Content, FileContentType(s.File.Name), s.File.Name);
        });
        api.MapPost("/{id}/versions/{version}/restore", async (string id, string version, HttpRequest req, SessionStore store, RoomManager rooms, AuditService audit, TimeProvider time, CancellationToken ct) =>
        {
            var s = RequireEdit(id, req, store, time);
            var result = await s.Storage.RestoreVersionAsync(s.Context, s.FileId, version, ct);
            s.File = s.File with { Version = result.Version };
            if (rooms.Find(s.RoomKey) is { } room)
            {
                await room.ResetAsync(result.Version);
            }

            await audit.RecordAsync(AuditAction.Restore, ResourceKind.File, s.FileId, s.User.Id, s.User.Name, $"{version} -> {result.Version}", s.Host, ct);
            return Results.Ok(new { version = result.Version });
        });
        api.MapPost("/{id}/rename", async (string id, RenameRequest body, HttpRequest req, SessionStore store, TimeProvider time, CancellationToken ct) =>
        {
            var s = RequireEdit(id, req, store, time);
            var name = await s.Storage.RenameAsync(s.Context, s.FileId, Files.FileService.ValidateName(body.Name), ct);
            s.File = s.File with { Name = name };
            return Results.Ok(new { name });
        });
        api.MapPost("/{id}/save-as", async (string id, string name, string? folderId, HttpRequest req, SessionStore store, StorageOptions storage, TimeProvider time, CancellationToken ct) =>
        {
            var s = Require(id, req, store, time);
            var content = await ReadBodyAsync(req, storage.MaxUploadBytes, ct);
            var created = await s.Storage.CreateFileAsync(s.Context, folderId ?? "", Files.FileService.ValidateName(name), content, ct);
            return Results.Ok(created);
        }).DisableAntiforgery();

        // @mentions in comments (CORE-COMMENT-001): people picker and notifications of a posted comment.
        api.MapGet("/{id}/people", async (string id, string? q, int? limit, HttpRequest req, SessionStore store, CommentMentionService mentions, TimeProvider time, CancellationToken ct) =>
        {
            var s = Require(id, req, store, time);
            return Results.Ok(new { people = await mentions.SearchAsync(s, q, limit ?? 10, ct) });
        });
        api.MapPost("/{id}/mentions", async (string id, CommentMentionRequest body, HttpRequest req, SessionStore store, CommentMentionService mentions, TimeProvider time, CancellationToken ct) =>
            Results.Ok(await mentions.NotifyAsync(Require(id, req, store, time), body, ct)));

        // Offline edits (desktop/web cache) uploaded with the version they were based on; conflicts produce a copy.
        api.MapPost("/{id}/sync", async (string id, string baseVersion, HttpRequest req, SessionStore store, SessionService sessions, StorageOptions storage, TimeProvider time, CancellationToken ct) =>
        {
            var s = Require(id, req, store, time);
            var content = await ReadBodyAsync(req, storage.MaxUploadBytes, ct);
            return Results.Ok(await sessions.SyncOfflineAsync(s, content, baseVersion, ct));
        }).DisableAntiforgery();

        // Host → BlueOffice notifications, authenticated with a host-signed JWT.
        app.MapPost("/api/hosts/{host}/notify", async (string host, HostNotification body, ClaimsPrincipal p, SessionService sessions, CancellationToken ct) =>
        {
            if (!string.Equals(p.FindFirst(AuthSetup.HostClaim)?.Value, host, StringComparison.OrdinalIgnoreCase))
            {
                throw ApiException.Forbidden("Token was not issued by this host.");
            }

            var count = body.Event switch
            {
                "permissions-changed" or "file-changed" => await sessions.RevalidateAsync(host, body.FileId, body.UserId, ct),
                _ => throw ApiException.BadRequest("Unknown event."),
            };
            return Results.Accepted(value: new { sessions = count });
        }).RequireAuthorization(AuthSetup.HostPolicy);

        // Origins allowed to embed the editor (the web shell whitelists them for postMessage before the host is known;
        // the same list is sent as CSP frame-ancestors, so it is not secret).
        app.MapGet("/api/hosting/origins", (HostRegistry hosts) => Results.Ok(new
        {
            origins = hosts.All.SelectMany(h => h.AllowedOrigins).Where(o => !string.IsNullOrWhiteSpace(o) && o != "*").Distinct(StringComparer.Ordinal).ToList(),
        })).AllowAnonymous();

        // Collaboration WebSocket.
        app.Map("/collab/{fileId}", CollabAsync).AllowAnonymous();

        // WOPI discovery ([MS-WOPI] §3.1.5.1): lets WOPI hosts find the BlueOffice action URLs.
        app.MapGet("/hosting/discovery", (HttpRequest req, WopiProofKeys proofKeys) =>
        {
            var baseUrl = $"{req.Scheme}://{req.Host}{req.PathBase}";
            return Results.Text(WopiDiscovery(baseUrl, proofKeys), "application/xml", Encoding.UTF8);
        }).AllowAnonymous();
    }

    private static async Task<IResult> CreateFromQueryAsync(HttpRequest req, string? formToken, SessionService sessions, CancellationToken ct)
    {
        var q = req.Query;
        var token = formToken ?? (string?)q["token"] ?? q["access_token"];
        SessionInfo info;
        if (!string.IsNullOrEmpty(q["wopisrc"]))
        {
            info = await sessions.CreateWopiSessionAsync(q["wopisrc"]!, token, ct);
        }
        else if (!string.IsNullOrEmpty(q["host"]) && !string.IsNullOrEmpty(q["file"]))
        {
            info = await sessions.CreateHostSessionAsync(q["host"]!, q["file"]!, token, ct);
        }
        else
        {
            throw ApiException.BadRequest("Expected ?host=&file=&token= or ?WOPISrc=&access_token=.");
        }

        return Results.Ok(info);
    }

    private static async Task<IResult> CheckpointAsync(
        string id,
        bool? save,
        bool? overwrite,
        HttpRequest req,
        SessionStore store,
        RoomManager rooms,
        AuditService audit,
        IHostNotifier notifier,
        StorageOptions storage,
        TimeProvider time,
        CancellationToken ct)
    {
        var s = RequireEdit(id, req, store, time);
        var content = await ReadBodyAsync(req, storage.MaxUploadBytes, ct);
        if (rooms.Find(s.RoomKey) is { } room)
        {
            var r = await room.SetCheckpointAsync(s, content, save == true, overwrite == true, ct);
            return r.Outcome == SaveOutcome.Conflict
                ? Results.Conflict(new { outcome = "conflict", currentVersion = r.Version, message = r.Message })
                : Results.Ok(new { outcome = System.Text.Json.JsonNamingPolicy.CamelCase.ConvertName(r.Outcome.ToString()), version = r.Version, message = r.Message });
        }

        // No collaboration room (single user without WebSocket): write straight to the host.
        try
        {
            var result = await s.Storage.PutContentsAsync(s.Context, s.FileId, content,
                new HostPutOptions(overwrite == true ? null : s.File.Version, null, "Edited in BlueOffice by " + s.User.Name, [s.User.Id]), ct);
            s.File = s.File with { Version = result.Version };
            notifier.Publish(s.Host, HostEvents.FileSaved, s.FileId, new { version = result.Version, reason = "manual", editors = new[] { new { id = s.User.Id, name = s.User.Name } } });
            await audit.RecordAsync(AuditAction.Edit, ResourceKind.File, s.FileId, s.User.Id, s.User.Name, $"version {result.Version}", s.Host, ct);
            return Results.Ok(new { outcome = "saved", version = result.Version, message = (string?)null });
        }
        catch (HostConflictException e)
        {
            return Results.Conflict(new { outcome = "conflict", currentVersion = e.CurrentVersion, message = e.Message });
        }
    }

    private static async Task CollabAsync(HttpContext ctx, string fileId, SessionStore store, RoomManager rooms, TimeProvider time)
    {
        if (!ctx.WebSockets.IsWebSocketRequest)
        {
            ctx.Response.StatusCode = StatusCodes.Status400BadRequest;
            return;
        }

        EditorSession? s;
        var sid = (string?)ctx.Request.Query["session"];
        if (sid is not null)
        {
            s = store.Get(sid, ctx.Request.Query["token"], time.GetUtcNow().UtcDateTime);
        }
        else if (ctx.User.Identity?.IsAuthenticated == true && Guid.TryParse(fileId, out _))
        {
            // Local file opened directly with a user JWT (?access_token=).
            s = await ctx.RequestServices.GetRequiredService<SessionService>().CreateLocalSessionObjectAsync(ctx.User, fileId, ctx.RequestAborted);
        }
        else
        {
            s = null;
        }

        if (s is null)
        {
            ctx.Response.StatusCode = StatusCodes.Status401Unauthorized;
            return;
        }

        if (s.FileId != fileId)
        {
            ctx.Response.StatusCode = StatusCodes.Status404NotFound;
            return;
        }

        if (!s.Permissions.View)
        {
            ctx.Response.StatusCode = StatusCodes.Status403Forbidden;
            return;
        }

        using var socket = await ctx.WebSockets.AcceptWebSocketAsync();
        await rooms.RunConnectionAsync(s, socket, ctx.RequestAborted);
    }

    internal static EditorSession Require(string id, HttpRequest req, SessionStore store, TimeProvider time) =>
        store.Get(id, req.Headers[TokenHeader], time.GetUtcNow().UtcDateTime)
        ?? throw (MisdirectedTo(id, req) is { } owner
            ? new ApiException(StatusCodes.Status421MisdirectedRequest, $"Session belongs to instance '{owner}'.")
            : new ApiException(StatusCodes.Status401Unauthorized, "Invalid or expired session."));

    /// <summary>Instance named by a session id (<c>{instance}~{guid}</c>) when it is not this one: the proxy sent
    /// the request to the wrong <c>api</c> instance.</summary>
    private static string? MisdirectedTo(string id, HttpRequest req)
    {
        var cut = id.IndexOf(InstanceOptions.SessionSeparator, StringComparison.Ordinal);
        if (cut <= 0)
        {
            return null;
        }

        var owner = id[..cut];
        var self = req.HttpContext.RequestServices.GetRequiredService<RoomManager>().Ownership.InstanceId;
        return owner == self ? null : owner;
    }

    internal static EditorSession RequireEdit(string id, HttpRequest req, SessionStore store, TimeProvider time)
    {
        var s = Require(id, req, store, time);
        return s.Permissions.Edit ? s : throw ApiException.Forbidden("Edit permission required.");
    }

    private static async Task<byte[]> ReadBodyAsync(HttpRequest req, long limit, CancellationToken ct)
    {
        using var ms = new MemoryStream();
        using var hashing = new Files.HashingStream(req.Body, limit);
        await hashing.CopyToAsync(ms, ct);
        return ms.ToArray();
    }

    private static string FileContentType(string name) => Files.FileService.ContentTypeOf(name);

    private static string WopiDiscovery(string baseUrl, WopiProofKeys proofKeys)
    {
        // [MS-WOPI] §3.1.5.1.1.1 wopi-discovery/net-zone/app/action; app names follow ADR-0006.
        var url = System.Security.SecurityElement.Escape(baseUrl + "/edit?<ui=UI_LLCC&>");
        var icon = System.Security.SecurityElement.Escape(baseUrl) + "/favicon.ico";
        var apps = new (string Name, string[] Ext)[]
        {
            ("BlueDoc", ["docx", "doc", "odt"]),
            ("BlueSheet", ["xlsx", "xls", "ods"]),
            ("BlueSlide", ["pptx", "ppt", "odp"]),
        };
        var sb = new StringBuilder();
        sb.Append("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<wopi-discovery>\n  <net-zone name=\"external-https\">\n");
        foreach (var (name, exts) in apps)
        {
            sb.Append($"    <app name=\"{name}\" favIconUrl=\"{icon}\">\n");
            foreach (var ext in exts)
            {
                sb.Append($"      <action name=\"view\" ext=\"{ext}\" default=\"true\" urlsrc=\"{url}\" />\n");
                sb.Append($"      <action name=\"edit\" ext=\"{ext}\" requires=\"locks,update\" urlsrc=\"{url}\" />\n");
            }

            sb.Append("    </app>\n");
        }

        sb.Append("  </net-zone>\n");

        // Public proof keys ([MS-WOPI] §3.1.5.1.1.1 proof-key): hosts verify X-WOPI-Proof / X-WOPI-ProofOld with them.
        sb.Append("  ").Append(proofKeys.DiscoveryElement().ToString(System.Xml.Linq.SaveOptions.DisableFormatting)).Append('\n');
        sb.Append("</wopi-discovery>\n");
        return sb.ToString();
    }
}
