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
        app.MapGet("/_admin/webhooks", (MockState s) =>
        {
            lock (s)
            {
                return Results.Ok(s.Webhooks.ToList());
            }
        });
        app.MapPost("/webhooks/blueoffice", async (HttpRequest req, MockState s) =>
        {
            var body = await ReadAllAsync(req.Body);
            var ts = req.Headers["X-BlueOffice-Timestamp"].ToString();
            var sig = req.Headers["X-BlueOffice-Signature"].ToString();
            var expected = "sha256=" + Convert.ToHexString(HMACSHA256.HashData(
                Encoding.UTF8.GetBytes(s.WebhookSecret), Encoding.UTF8.GetBytes(ts + ".").Concat(body).ToArray())).ToLowerInvariant();
            using var doc = JsonDocument.Parse(body);
            lock (s)
            {
                s.Webhooks.Add(new WebhookRecord(
                    req.Headers["X-BlueOffice-Event"].ToString(),
                    doc.RootElement.GetProperty("fileId").GetString() ?? "",
                    Encoding.UTF8.GetString(body),
                    CryptographicOperations.FixedTimeEquals(Encoding.ASCII.GetBytes(sig), Encoding.ASCII.GetBytes(expected))));
            }

            return Results.Ok();
        });

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
            var (_, error) = await s.AuthorizeAsync(req, id, p => p.View);
            if (error is not null)
            {
                return error;
            }

            lock (s)
            {
                var f = s.Get(id);
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

/// <summary>A webhook received from BlueOffice.</summary>
public sealed record WebhookRecord(string Event, string FileId, string Body, bool SignatureValid);

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
        CreateFile("Báo cáo.docx", "u1", Encoding.UTF8.GetBytes("seed"), "root");
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
