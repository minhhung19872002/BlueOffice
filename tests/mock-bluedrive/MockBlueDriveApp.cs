using System.Globalization;
using System.Net;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using Microsoft.IdentityModel.JsonWebTokens;
using Microsoft.IdentityModel.Tokens;

namespace MockBlueDrive;

/// <summary>Entry point: <c>dotnet run --project tests/mock-bluedrive</c> (http://localhost:5080).</summary>
public static class MockBlueDriveProgram
{
    /// <summary>Main.</summary>
    public static void Main(string[] args) => MockBlueDriveApp.Build(args).Run();
}

/// <summary>
/// Minimal BlueDrive implementing the BlueOffice Host Protocol (docs/bohp.md) for integration and E2E tests:
/// in-memory files/versions/locks, RS256 tokens with a JWKS endpoint, webhook receiver and admin endpoints.
/// </summary>
public static class MockBlueDriveApp
{
    /// <summary>Builds the app; <paramref name="configure"/> lets tests use a TestServer.</summary>
    public static WebApplication Build(string[] args, Action<WebApplicationBuilder>? configure = null)
    {
        var builder = WebApplication.CreateBuilder(args);
        configure?.Invoke(builder);
        builder.Services.AddSingleton(sp => new MockState(sp.GetRequiredService<IConfiguration>()));
        var app = builder.Build();
        Map(app);
        return app;
    }

    private static void Map(WebApplication app)
    {
        app.MapGet("/.well-known/jwks.json", (MockState s) => Results.Json(s.Jwks()));

        // ---------------------------------------------------------------- admin (test control)
        app.MapPost("/_admin/token", (TokenRequest r, MockState s) =>
            Results.Ok(new { accessToken = s.IssueToken(r.UserId, r.FileId, r.TtlSeconds ?? 600) }));
        app.MapPost("/_admin/files", (CreateFileRequest r, MockState s) =>
        {
            var f = s.CreateFile(r.Name, r.OwnerId ?? "u1", r.Content is null ? [] : Convert.FromBase64String(r.Content), r.FolderId ?? "root");
            return Results.Ok(new { id = f.Id, version = f.Version });
        });
        app.MapPut("/_admin/files/{id}/permissions/{userId}", (string id, string userId, Perms p, MockState s) =>
        {
            lock (s)
            {
                s.Get(id).Permissions[userId] = p;
            }

            return Results.NoContent();
        });
        app.MapPut("/_admin/files/{id}/contents", async (string id, HttpRequest req, MockState s) =>
        {
            // External change (e.g. upload from the BlueDrive UI) → new version.
            var bytes = await ReadAllAsync(req.Body);
            lock (s)
            {
                var f = s.Get(id);
                f.AddVersion(bytes, "u1", "External upload", []);
                return Results.Ok(new { version = f.Version });
            }
        });
        app.MapGet("/_admin/files/{id}", (string id, MockState s) =>
        {
            lock (s)
            {
                var f = s.Get(id);
                return Results.Ok(new
                {
                    f.Id,
                    f.Name,
                    f.Version,
                    lockId = f.LockActive(s.Now) ? f.LockId : null,
                    lockExpiresAt = f.LockActive(s.Now) ? f.LockExpires : (DateTime?)null,
                    versions = f.Versions.Select(v => new { v.Version, size = v.Content.Length, v.ModifiedBy, v.Note, v.Editors, content = Convert.ToBase64String(v.Content) }),
                });
            }
        });
        app.MapGet("/_admin/folders/{folderId}/files", (string folderId, MockState s) =>
        {
            lock (s)
            {
                return Results.Ok(s.Files.Values.Where(f => f.FolderId == folderId)
                    .Select(f => new { id = f.Id, name = f.Name, version = f.Version, ownerId = f.OwnerId }).ToList());
            }
        });
        app.MapPost("/_admin/folders", (CreateFolderRequest r, MockState s) =>
        {
            var f = s.CreateFolder(r.Name, r.ParentId ?? "root", r.OwnerId ?? "u1");
            return Results.Ok(new { id = f.Id, name = f.Name });
        });
        app.MapGet("/_admin/webhooks", (MockState s) =>
        {
            lock (s)
            {
                return Results.Ok(s.Webhooks.ToList());
            }
        });
        app.MapPost("/_admin/webhooks/fail", (FailRequest r, MockState s) =>
        {
            // The next `Count` deliveries are answered with `Status` (retry tests).
            lock (s)
            {
                s.WebhookFailures = Math.Max(0, r.Count);
                s.WebhookFailureStatus = r.Status ?? 503;
            }

            return Results.NoContent();
        });
        app.MapPost("/webhooks/blueoffice", async (HttpRequest req, MockState s) =>
        {
            var body = await ReadAllAsync(req.Body);
            var ts = req.Headers["X-BlueOffice-Timestamp"].ToString();
            var sig = req.Headers["X-BlueOffice-Signature"].ToString();
            var expected = "sha256=" + Convert.ToHexString(HMACSHA256.HashData(
                Encoding.UTF8.GetBytes(s.WebhookSecret), Encoding.UTF8.GetBytes(ts + ".").Concat(body).ToArray())).ToLowerInvariant();

            // Replay protection recommended by docs/bohp.md §D: timestamp within ±5 minutes.
            var fresh = long.TryParse(ts, NumberStyles.Integer, CultureInfo.InvariantCulture, out var unix)
                && Math.Abs(DateTimeOffset.UtcNow.ToUnixTimeSeconds() - unix) <= 300;
            using var doc = JsonDocument.Parse(body);
            lock (s)
            {
                s.WebhookAttempts++;
                if (s.WebhookFailures > 0)
                {
                    s.WebhookFailures--;
                    return Results.StatusCode(s.WebhookFailureStatus);
                }

                s.Webhooks.Add(new WebhookRecord(
                    req.Headers["X-BlueOffice-Event"].ToString(),
                    doc.RootElement.GetProperty("fileId").GetString() ?? "",
                    Encoding.UTF8.GetString(body),
                    CryptographicOperations.FixedTimeEquals(Encoding.ASCII.GetBytes(sig), Encoding.ASCII.GetBytes(expected)) && fresh,
                    s.WebhookAttempts));
            }

            return Results.Ok();
        });

        // ---------------------------------------------------------------- sign-in (OAuth 2.0 code + PKCE, RFC 7636)
        app.MapGet("/oauth/authorize", (HttpRequest req, MockState s) => s.Authorize(req));
        app.MapPost("/oauth/token", async (HttpRequest req, HttpResponse res, MockState s) =>
        {
            // The desktop web view exchanges the code itself (a form POST is a CORS "simple request").
            res.Headers.AccessControlAllowOrigin = "*";
            return s.ExchangeCode(await req.ReadFormAsync());
        }).DisableAntiforgery();

        // ---------------------------------------------------------------- BOHP v1
        var bohp = app.MapGroup("/bohp/v1");
        bohp.MapGet("/files/{id}", async (string id, HttpRequest req, MockState s) =>
        {
            var (user, error) = await s.AuthorizeAsync(req, id, p => p.View);
            if (error is not null)
            {
                return error;
            }

            lock (s)
            {
                var f = s.Get(id);
                var u = s.Users[user!];
                return Results.Ok(new
                {
                    id = f.Id,
                    name = f.Name,
                    size = f.Current.Content.Length,
                    version = f.Version,
                    ownerId = f.OwnerId,
                    folderId = f.FolderId,
                    lastModified = f.Current.At,
                    shareUrl = $"https://drive.mock/s/{f.Id}",
                    permissions = f.PermsOf(user!),
                    user = new { id = user, name = u, avatarUrl = (string?)null },
                    @lock = f.LockActive(s.Now) ? new { lockId = f.LockId, expiresAt = f.LockExpires } : null,
                });
            }
        });
        bohp.MapGet("/files/{id}/contents", async (string id, HttpRequest req, HttpResponse res, MockState s) =>
        {
            var (user, error) = await s.AuthorizeAsync(req, id, p => p.View);
            if (error is not null)
            {
                return error;
            }

            lock (s)
            {
                var f = s.Get(id);
                s.TouchRecent(user!, id);
                res.Headers.ETag = $"\"{f.Version}\"";
                res.Headers["X-BOHP-Version"] = f.Version;
                return Results.Bytes(f.Current.Content, "application/octet-stream");
            }
        });
        bohp.MapPost("/files/{id}/contents", async (string id, HttpRequest req, HttpResponse res, MockState s) =>
        {
            var (user, error) = await s.AuthorizeAsync(req, id, p => p.Edit);
            if (error is not null)
            {
                return error;
            }

            var body = await ReadAllAsync(req.Body);
            lock (s)
            {
                var f = s.Get(id);
                var lockHeader = req.Headers["X-BOHP-Lock"].ToString();
                if (f.LockActive(s.Now) && f.LockId != lockHeader)
                {
                    return Results.Conflict(new { error = "lock-conflict", lockId = f.LockId, currentVersion = f.Version });
                }

                var ifMatch = req.Headers.IfMatch.ToString().Trim('"', ' ');
                if (ifMatch.Length > 0 && ifMatch != f.Version)
                {
                    return Results.Conflict(new { error = "version-conflict", currentVersion = f.Version });
                }

                var note = Uri.UnescapeDataString(req.Headers["X-BOHP-Note"].ToString());
                var editors = req.Headers["X-BOHP-Editors"].ToString().Split(',', StringSplitOptions.RemoveEmptyEntries).Select(Uri.UnescapeDataString).ToList();
                f.AddVersion(body, user!, note, editors);
                res.Headers.ETag = $"\"{f.Version}\"";
                return Results.Ok(new { version = f.Version });
            }
        });
        bohp.MapPost("/files/{id}/lock", (string id, LockRequest r, HttpRequest req, MockState s) => LockOpAsync(id, r, req, s, "lock"));
        bohp.MapPost("/files/{id}/refresh-lock", (string id, LockRequest r, HttpRequest req, MockState s) => LockOpAsync(id, r, req, s, "refresh"));
        bohp.MapPost("/files/{id}/unlock", (string id, LockRequest r, HttpRequest req, MockState s) => LockOpAsync(id, r, req, s, "unlock"));
        bohp.MapGet("/files/{id}/versions", async (string id, HttpRequest req, MockState s) =>
        {
            var (_, error) = await s.AuthorizeAsync(req, id, p => p.View);
            if (error is not null)
            {
                return error;
            }

            lock (s)
            {
                var f = s.Get(id);
                return Results.Ok(new
                {
                    versions = f.Versions.AsEnumerable().Reverse().Select(v => new
                    {
                        version = v.Version,
                        size = v.Content.Length,
                        modifiedAt = v.At,
                        modifiedBy = new { id = v.ModifiedBy, name = s.Users.GetValueOrDefault(v.ModifiedBy, v.ModifiedBy) },
                        note = v.Note,
                    }),
                });
            }
        });
        bohp.MapGet("/files/{id}/versions/{v}/contents", async (string id, string v, HttpRequest req, MockState s) =>
        {
            var (_, error) = await s.AuthorizeAsync(req, id, p => p.View);
            if (error is not null)
            {
                return error;
            }

            lock (s)
            {
                var old = s.Get(id).Versions.FirstOrDefault(x => x.Version == v);
                if (old is null)
                {
                    return Results.NotFound();
                }

                req.HttpContext.Response.Headers.ETag = $"\"{old.Version}\"";
                return Results.Bytes(old.Content, "application/octet-stream");
            }
        });
        bohp.MapPost("/files/{id}/versions/{v}/restore", async (string id, string v, HttpRequest req, MockState s) =>
        {
            var (user, error) = await s.AuthorizeAsync(req, id, p => p.Edit);
            if (error is not null)
            {
                return error;
            }

            lock (s)
            {
                var f = s.Get(id);
                var old = f.Versions.FirstOrDefault(x => x.Version == v);
                if (old is null)
                {
                    return Results.NotFound();
                }

                f.AddVersion(old.Content, user!, $"Restored from version {v}", []);
                return Results.Ok(new { version = f.Version });
            }
        });
        bohp.MapPost("/files/{id}/rename", async (string id, RenameRequest r, HttpRequest req, MockState s) =>
        {
            var (_, error) = await s.AuthorizeAsync(req, id, p => p.Edit);
            if (error is not null)
            {
                return error;
            }

            lock (s)
            {
                var f = s.Get(id);
                if (s.Files.Values.Any(x => x != f && x.FolderId == f.FolderId && x.Name == r.Name))
                {
                    return Results.Conflict(new { error = "name-conflict" });
                }

                f.Name = r.Name;
                return Results.Ok(new { name = f.Name });
            }
        });
        bohp.MapPost("/folders/{folderId}/files", async (string folderId, string name, HttpRequest req, MockState s) =>
        {
            var (user, error) = await s.AuthorizeAsync(req, null, null);
            if (error is not null)
            {
                return error;
            }

            var body = await ReadAllAsync(req.Body);
            lock (s)
            {
                var f = s.CreateFile(name, user!, body, folderId);
                return Results.Created($"/bohp/v1/files/{f.Id}", new { id = f.Id, name = f.Name, version = f.Version });
            }
        });

        // ---------------------------------------------------------------- BOHP §A2 browsing (user tokens without `fid`)
        bohp.MapGet("/me", async (HttpRequest req, MockState s) =>
        {
            var (user, error) = await s.AuthorizeUserAsync(req);
            return error ?? Results.Ok(new { id = user, name = s.Users.GetValueOrDefault(user!, user!), avatarUrl = (string?)null });
        });
        bohp.MapGet("/folders/{folderId}/children", async (string folderId, HttpRequest req, MockState s) =>
        {
            var (user, error) = await s.AuthorizeUserAsync(req);
            if (error is not null)
            {
                return error;
            }

            lock (s)
            {
                if (!s.Folders.TryGetValue(folderId, out var folder))
                {
                    return Results.NotFound();
                }

                var folders = s.Folders.Values.Where(f => f.ParentId == folderId).OrderBy(f => f.Name, StringComparer.Ordinal)
                    .Select(f => (object)new { kind = "folder", id = f.Id, name = f.Name });
                var files = s.Files.Values.Where(f => f.FolderId == folderId && f.PermsOf(user!).View).OrderBy(f => f.Name, StringComparer.Ordinal)
                    .Select(f => s.Item(f));
                return Results.Ok(new
                {
                    folder = new { id = folder.Id, name = folder.Name, parentId = folder.ParentId },
                    items = folders.Concat(files).ToList(),
                });
            }
        });
        bohp.MapGet("/shared", async (HttpRequest req, MockState s) =>
        {
            var (user, error) = await s.AuthorizeUserAsync(req);
            if (error is not null)
            {
                return error;
            }

            lock (s)
            {
                return Results.Ok(new
                {
                    items = s.Files.Values.Where(f => f.OwnerId != user && f.Permissions.TryGetValue(user!, out var p) && p.View)
                        .OrderBy(f => f.Name, StringComparer.Ordinal).Select(f => s.Item(f)).ToList(),
                });
            }
        });
        bohp.MapGet("/me/recent", async (int? limit, HttpRequest req, MockState s) =>
        {
            var (user, error) = await s.AuthorizeUserAsync(req);
            if (error is not null)
            {
                return error;
            }

            lock (s)
            {
                var recent = s.Recent.GetValueOrDefault(user!) ?? [];
                return Results.Ok(new
                {
                    items = recent.Where(r => s.Files.TryGetValue(r.FileId, out var f) && f.PermsOf(user!).View)
                        .Take(Math.Clamp(limit ?? 20, 1, 100))
                        .Select(r => s.Item(s.Files[r.FileId], r.At)).ToList(),
                });
            }
        });
        bohp.MapGet("/sync/resolve", async (string? path, HttpRequest req, MockState s) =>
        {
            // The mock's sync folder mirrors the folder tree under "root": "Dự án/Kế hoạch.docx".
            var (user, error) = await s.AuthorizeUserAsync(req);
            if (error is not null)
            {
                return error;
            }

            lock (s)
            {
                var f = s.ResolvePath(path ?? "");
                return f is null || !f.PermsOf(user!).View ? Results.NotFound() : Results.Ok(new { id = f.Id, name = f.Name, version = f.Version });
            }
        });
    }

    private static async Task<IResult> LockOpAsync(string id, LockRequest r, HttpRequest req, MockState s, string op)
    {
        var (_, error) = await s.AuthorizeAsync(req, id, p => p.Edit);
        if (error is not null)
        {
            return error;
        }

        lock (s)
        {
            var f = s.Get(id);
            var active = f.LockActive(s.Now);
            switch (op)
            {
                case "lock" when active && f.LockId != r.LockId:
                case "refresh" when !active || f.LockId != r.LockId:
                case "unlock" when active && f.LockId != r.LockId:
                    return Results.Conflict(new { error = "lock-conflict", lockId = active ? f.LockId : null });
                case "unlock":
                    f.LockId = null;
                    return Results.Ok(new { lockId = r.LockId });
                default:
                    f.LockId = r.LockId;
                    f.LockExpires = s.Now.AddSeconds(Math.Clamp(r.ExpiresInSeconds ?? 1800, 60, 3600));
                    return Results.Ok(new { lockId = f.LockId, expiresAt = f.LockExpires });
            }
        }
    }

    private static async Task<byte[]> ReadAllAsync(Stream s)
    {
        using var ms = new MemoryStream();
        await s.CopyToAsync(ms);
        return ms.ToArray();
    }
}

/// <summary>POST /_admin/token.</summary>
public sealed record TokenRequest(string UserId, string? FileId, int? TtlSeconds);

/// <summary>POST /_admin/files.</summary>
public sealed record CreateFileRequest(string Name, string? OwnerId, string? Content, string? FolderId);

/// <summary>BOHP lock body.</summary>
public sealed record LockRequest(string LockId, int? ExpiresInSeconds);

/// <summary>BOHP rename body.</summary>
public sealed record RenameRequest(string Name);

/// <summary>BOHP permissions.</summary>
public sealed record Perms(bool View, bool Comment, bool Edit, bool Share, bool Download);

/// <summary>POST /_admin/folders.</summary>
public sealed record CreateFolderRequest(string Name, string? ParentId, string? OwnerId);

/// <summary>POST /_admin/webhooks/fail.</summary>
public sealed record FailRequest(int Count, int? Status);

/// <summary>A webhook received from BlueOffice (<c>Attempt</c> = deliveries seen by the receiver so far, refused ones included).</summary>
public sealed record WebhookRecord(string Event, string FileId, string Body, bool SignatureValid, int Attempt);

/// <summary>A folder.</summary>
public sealed record MockFolder(string Id, string Name, string? ParentId, string OwnerId);

/// <summary>A file opened by a user.</summary>
public sealed record RecentOpen(string FileId, DateTime At);

/// <summary>A pending authorization code.</summary>
public sealed record AuthCode(string User, string Challenge, string RedirectUri, string ClientId, DateTime Expires);

/// <summary>A stored version.</summary>
public sealed record MockVersion(string Version, byte[] Content, string ModifiedBy, string? Note, IReadOnlyList<string> Editors, DateTime At);

/// <summary>A file.</summary>
public sealed class MockFile
{
    /// <summary>Id.</summary>
    public string Id { get; init; } = "";

    /// <summary>Name.</summary>
    public string Name { get; set; } = "";

    /// <summary>Folder.</summary>
    public string FolderId { get; init; } = "root";

    /// <summary>Owner.</summary>
    public string OwnerId { get; init; } = "";

    /// <summary>Versions (oldest first).</summary>
    public List<MockVersion> Versions { get; } = [];

    /// <summary>User → permissions (owner has everything).</summary>
    public Dictionary<string, Perms> Permissions { get; } = [];

    /// <summary>Lock id.</summary>
    public string? LockId { get; set; }

    /// <summary>Lock expiry.</summary>
    public DateTime LockExpires { get; set; }

    /// <summary>Current version.</summary>
    public MockVersion Current => Versions[^1];

    /// <summary>Current version id.</summary>
    public string Version => Current.Version;

    /// <summary>True if locked.</summary>
    public bool LockActive(DateTime now) => LockId is not null && LockExpires > now;

    /// <summary>Permissions of a user.</summary>
    public Perms PermsOf(string user) =>
        user == OwnerId ? new Perms(true, true, true, true, true) : Permissions.GetValueOrDefault(user, new Perms(false, false, false, false, false));

    /// <summary>Appends a version.</summary>
    public void AddVersion(byte[] content, string by, string? note, IReadOnlyList<string> editors) =>
        Versions.Add(new MockVersion((Versions.Count + 1).ToString(System.Globalization.CultureInfo.InvariantCulture), content, by, note, editors, DateTime.UtcNow));
}

/// <summary>All mock state (lock on the instance when mutating).</summary>
public sealed class MockState
{
    private readonly RSA _rsa = RSA.Create(2048);
    private readonly RsaSecurityKey _key;
    private readonly JsonWebTokenHandler _handler = new();
    private int _nextId;

    /// <summary>Creates the state from configuration (<c>Mock:Issuer</c>, <c>Mock:Audience</c>, <c>Mock:WebhookSecret</c>).</summary>
    public MockState(IConfiguration config)
    {
        Issuer = config["Mock:Issuer"] ?? "http://mock-bluedrive";
        Audience = config["Mock:Audience"] ?? "blueoffice";
        WebhookSecret = config["Mock:WebhookSecret"] ?? "mock-webhook-secret";
        _key = new RsaSecurityKey(_rsa) { KeyId = "mock-key-1" };
        Folders["root"] = new MockFolder("root", "BlueDrive", null, "u1");
        CreateFile("Báo cáo.docx", "u1", Encoding.UTF8.GetBytes("seed"), "root");
    }

    /// <summary>Folders (one tree shared by all mock users; "root" is the top).</summary>
    public Dictionary<string, MockFolder> Folders { get; } = [];

    /// <summary>Recently opened files per user, newest first.</summary>
    public Dictionary<string, List<RecentOpen>> Recent { get; } = [];

    /// <summary>Webhook deliveries to refuse next.</summary>
    public int WebhookFailures { get; set; }

    /// <summary>Status used for refused deliveries.</summary>
    public int WebhookFailureStatus { get; set; } = 503;

    /// <summary>Webhook deliveries received (accepted or refused).</summary>
    public int WebhookAttempts { get; set; }

    private Dictionary<string, AuthCode> Codes { get; } = [];

    /// <summary>Creates a folder.</summary>
    public MockFolder CreateFolder(string name, string parentId, string owner)
    {
        lock (this)
        {
            var f = new MockFolder($"d{++_nextId}", name, parentId, owner);
            Folders[f.Id] = f;
            return f;
        }
    }

    /// <summary>Records that <paramref name="user"/> opened <paramref name="fileId"/> (caller holds the lock).</summary>
    public void TouchRecent(string user, string fileId)
    {
        if (!Recent.TryGetValue(user, out var list))
        {
            Recent[user] = list = [];
        }

        list.RemoveAll(r => r.FileId == fileId);
        list.Insert(0, new RecentOpen(fileId, Now));
    }

    /// <summary>Listing entry of a file (caller holds the lock).</summary>
    public object Item(MockFile f, DateTime? openedAt = null) => new
    {
        kind = "file",
        id = f.Id,
        name = f.Name,
        size = f.Current.Content.Length,
        version = f.Version,
        lastModified = f.Current.At,
        ownerId = f.OwnerId,
        openedAt,
    };

    /// <summary>File at a '/'-separated path under "root" (caller holds the lock).</summary>
    public MockFile? ResolvePath(string path)
    {
        var parts = path.Replace('\\', '/').Split('/', StringSplitOptions.RemoveEmptyEntries);
        if (parts.Length == 0)
        {
            return null;
        }

        var folder = "root";
        foreach (var name in parts[..^1])
        {
            var next = Folders.Values.FirstOrDefault(f => f.ParentId == folder && string.Equals(f.Name, name, StringComparison.OrdinalIgnoreCase));
            if (next is null)
            {
                return null;
            }

            folder = next.Id;
        }

        return Files.Values.FirstOrDefault(f => f.FolderId == folder && string.Equals(f.Name, parts[^1], StringComparison.OrdinalIgnoreCase));
    }

    /// <summary>
    /// Mock authorization endpoint: signs in <c>login_hint</c> (a mock user id) without a password and redirects back with a
    /// code; without a hint it shows a page with one link per user.
    /// </summary>
    public IResult Authorize(HttpRequest req)
    {
        var q = req.Query;
        string redirect = q["redirect_uri"].ToString(), state = q["state"].ToString(), challenge = q["code_challenge"].ToString();
        if (q["response_type"] != "code" || q["code_challenge_method"] != "S256" || challenge.Length < 43 || redirect.Length == 0)
        {
            return Results.BadRequest(new { error = "invalid_request" });
        }

        var hint = q["login_hint"].ToString();
        if (hint.Length == 0 || !Users.ContainsKey(hint))
        {
            var links = string.Concat(Users.Keys.Where(u => u != "bluedrive-service").Select(u =>
                $"<li><a href=\"{WebUtility.HtmlEncode(req.Path + req.QueryString.Add("login_hint", u).ToUriComponent())}\">{WebUtility.HtmlEncode(Users[u])}</a></li>"));
            return Results.Content($"<!doctype html><meta charset=utf-8><title>BlueDrive (mock)</title><ul>{links}</ul>", "text/html; charset=utf-8");
        }

        var code = Base64UrlEncoder.Encode(RandomNumberGenerator.GetBytes(24));
        lock (this)
        {
            Codes[code] = new AuthCode(hint, challenge, redirect, q["client_id"].ToString(), Now.AddMinutes(2));
        }

        var sep = redirect.Contains('?', StringComparison.Ordinal) ? '&' : '?';
        return Results.Redirect($"{redirect}{sep}code={Uri.EscapeDataString(code)}&state={Uri.EscapeDataString(state)}");
    }

    /// <summary>Mock token endpoint (authorization_code grant with PKCE S256, RFC 7636 §4.6).</summary>
    public IResult ExchangeCode(IFormCollection form)
    {
        AuthCode? entry;
        lock (this)
        {
            if (!Codes.Remove(form["code"].ToString(), out entry))
            {
                return Results.BadRequest(new { error = "invalid_grant" });
            }
        }

        var challenge = Base64UrlEncoder.Encode(SHA256.HashData(Encoding.ASCII.GetBytes(form["code_verifier"].ToString())));
        if (form["grant_type"] != "authorization_code" || entry.Expires < Now || form["redirect_uri"] != entry.RedirectUri
            || form["client_id"] != entry.ClientId
            || !CryptographicOperations.FixedTimeEquals(Encoding.ASCII.GetBytes(challenge), Encoding.ASCII.GetBytes(entry.Challenge)))
        {
            return Results.BadRequest(new { error = "invalid_grant" });
        }

        return Results.Ok(new { access_token = IssueToken(entry.User, null, 600), token_type = "Bearer", expires_in = 600 });
    }

    /// <summary>Validates a user token that is not scoped to one file (browsing endpoints).</summary>
    public async Task<(string? User, IResult? Error)> AuthorizeUserAsync(HttpRequest req)
    {
        var (user, error) = await AuthorizeAsync(req, null, null);
        if (error is not null)
        {
            return (null, error);
        }

        var jwt = new JsonWebToken(req.Headers.Authorization.ToString()[7..]);
        return jwt.TryGetPayloadValue<string>("fid", out _) ? (null, Results.StatusCode(403)) : (user, null);
    }

    /// <summary>Token issuer.</summary>
    public string Issuer { get; }

    /// <summary>Token audience.</summary>
    public string Audience { get; }

    /// <summary>Webhook secret.</summary>
    public string WebhookSecret { get; }

    /// <summary>Users.</summary>
    public Dictionary<string, string> Users { get; } = new()
    {
        ["u1"] = "Nguyễn Văn An",
        ["u2"] = "Trần Thị Bình",
        ["u3"] = "Lê Văn Cường",
        ["bluedrive-service"] = "BlueDrive",
    };

    /// <summary>Files.</summary>
    public Dictionary<string, MockFile> Files { get; } = [];

    /// <summary>Received webhooks.</summary>
    public List<WebhookRecord> Webhooks { get; } = [];

    /// <summary>Now (UTC).</summary>
    public DateTime Now => DateTime.UtcNow;

    /// <summary>File by id (throws if missing).</summary>
    public MockFile Get(string id) => Files.TryGetValue(id, out var f) ? f : throw new BadHttpRequestException("No such file", 404);

    /// <summary>Creates a file with version 1.</summary>
    public MockFile CreateFile(string name, string owner, byte[] content, string folderId)
    {
        lock (this)
        {
            var f = new MockFile { Id = $"f{++_nextId}", Name = name, OwnerId = owner, FolderId = folderId };
            f.AddVersion(content, owner, "Created", []);
            Files[f.Id] = f;
            return f;
        }
    }

    /// <summary>Issues an RS256 access token.</summary>
    public string IssueToken(string userId, string? fileId, int ttlSeconds)
    {
        var claims = new Dictionary<string, object> { ["sub"] = userId, ["name"] = Users.GetValueOrDefault(userId, userId) };
        if (fileId is not null)
        {
            claims["fid"] = fileId;
        }

        return _handler.CreateToken(new SecurityTokenDescriptor
        {
            Issuer = Issuer,
            Audience = Audience,
            Claims = claims,
            NotBefore = Now.AddSeconds(-5),
            Expires = Now.AddSeconds(ttlSeconds),
            SigningCredentials = new SigningCredentials(_key, SecurityAlgorithms.RsaSha256),
        });
    }

    /// <summary>JWKS document.</summary>
    public object Jwks()
    {
        var p = _rsa.ExportParameters(false);
        return new
        {
            keys = new[]
            {
                new { kty = "RSA", use = "sig", alg = "RS256", kid = _key.KeyId, n = Base64UrlEncoder.Encode(p.Modulus), e = Base64UrlEncoder.Encode(p.Exponent) },
            },
        };
    }

    /// <summary>Validates the bearer token and the permission; returns (user, null) or (null, error result).</summary>
    public async Task<(string? User, IResult? Error)> AuthorizeAsync(HttpRequest req, string? fileId, Func<Perms, bool>? check)
    {
        var header = req.Headers.Authorization.ToString();
        if (!header.StartsWith("Bearer ", StringComparison.Ordinal))
        {
            return (null, Results.Unauthorized());
        }

        var result = await _handler.ValidateTokenAsync(header[7..], new TokenValidationParameters
        {
            ValidIssuer = Issuer,
            ValidAudience = Audience,
            IssuerSigningKey = _key,
            ClockSkew = TimeSpan.FromSeconds(30),
        });
        if (!result.IsValid)
        {
            return (null, Results.Unauthorized());
        }

        var jwt = (JsonWebToken)result.SecurityToken;
        var user = jwt.Subject;
        if (jwt.TryGetPayloadValue<string>("fid", out var fid) && fileId is not null && fid != fileId)
        {
            return (null, Results.StatusCode(403));
        }

        if (fileId is null || check is null)
        {
            return (user, null);
        }

        lock (this)
        {
            if (!Files.TryGetValue(fileId, out var f))
            {
                return (null, Results.NotFound());
            }

            return check(f.PermsOf(user)) ? (user, null) : (null, Results.StatusCode(403));
        }
    }
}
