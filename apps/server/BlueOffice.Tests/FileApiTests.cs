using System.Net;
using System.Net.Http.Json;
using System.Security.Cryptography;
using System.Text.Json;
using BlueOffice.Domain;
using Xunit;

namespace BlueOffice.Tests;

/// <summary>Shared app for API tests (each test uses its own users and files).</summary>
public sealed class AppFixture : IDisposable
{
    /// <summary>The app.</summary>
    public TestApp App { get; } = new();

    /// <inheritdoc />
    public void Dispose() => App.Dispose();
}

internal static class Api
{
    public static async Task<JsonElement> UploadAsync(HttpClient c, string name, byte[] content, Guid? folderId = null)
    {
        var url = $"/api/files?name={Uri.EscapeDataString(name)}" + (folderId is { } f ? $"&folderId={f}" : "");
        var res = await c.PostAsync(new Uri(url, UriKind.Relative), new ByteArrayContent(content));
        Assert.Equal(HttpStatusCode.Created, res.StatusCode);
        return await res.Content.ReadFromJsonAsync<JsonElement>();
    }

    public static async Task<HttpStatusCode> ShareAsync(HttpClient owner, string kind, Guid id, string user, string role)
    {
        var res = await owner.PostAsJsonAsync(new Uri($"/api/{kind}/{id}/shares", UriKind.Relative), new { user, role });
        return res.StatusCode;
    }

    public static Guid Id(this JsonElement e) => e.GetProperty("id").GetGuid();
}

public sealed class PermissionMatrixTests(AppFixture fx) : IClassFixture<AppFixture>
{
    public static TheoryData<string, string, HttpStatusCode> Matrix()
    {
        var data = new TheoryData<string, string, HttpStatusCode>();
        var ok = HttpStatusCode.OK;
        var forbidden = HttpStatusCode.Forbidden;
        var expected = new Dictionary<string, (HttpStatusCode Viewer, HttpStatusCode Commenter, HttpStatusCode Editor, HttpStatusCode Owner)>
        {
            ["read"] = (ok, ok, ok, ok),
            ["download"] = (ok, ok, ok, ok),
            ["versions"] = (ok, ok, ok, ok),
            ["comment-audit"] = (HttpStatusCode.NoContent, HttpStatusCode.NoContent, HttpStatusCode.NoContent, HttpStatusCode.NoContent),
            ["edit"] = (forbidden, forbidden, ok, ok),
            ["rename"] = (forbidden, forbidden, ok, ok),
            ["restore"] = (forbidden, forbidden, ok, ok),
            ["shares"] = (forbidden, forbidden, forbidden, ok),
            ["audit"] = (forbidden, forbidden, forbidden, ok),
            ["delete"] = (forbidden, forbidden, forbidden, HttpStatusCode.NoContent),
        };
        foreach (var (op, e) in expected)
        {
            data.Add("viewer", op, e.Viewer);
            data.Add("commenter", op, e.Commenter);
            data.Add("editor", op, e.Editor);
            data.Add("owner", op, e.Owner);
            data.Add("none", op, HttpStatusCode.NotFound);
        }

        return data;
    }

    [Theory]
    [MemberData(nameof(Matrix))]
    public async Task RoleAllowsOnlyItsOperations(string role, string op, HttpStatusCode expected)
    {
        var ownerSub = "owner-" + Guid.NewGuid().ToString("N")[..8];
        var otherSub = "user-" + Guid.NewGuid().ToString("N")[..8];
        var owner = fx.App.ClientFor(ownerSub, "Chủ sở hữu");
        var other = fx.App.ClientFor(otherSub, "Người dùng");
        await other.GetAsync(new Uri("/api/me", UriKind.Relative)); // registers the user
        var file = await Api.UploadAsync(owner, "báo cáo.docx", [1, 2, 3]);
        var id = file.Id();
        var actor = role == "owner" ? owner : other;
        if (role is not ("owner" or "none"))
        {
            Assert.Equal(HttpStatusCode.OK, await Api.ShareAsync(owner, "files", id, otherSub, role));
        }

        var res = op switch
        {
            "read" => await actor.GetAsync(new Uri($"/api/files/{id}", UriKind.Relative)),
            "download" => await actor.GetAsync(new Uri($"/api/files/{id}/content", UriKind.Relative)),
            "versions" => await actor.GetAsync(new Uri($"/api/files/{id}/versions", UriKind.Relative)),
            "comment-audit" => await actor.PostAsJsonAsync(new Uri($"/api/files/{id}/audit", UriKind.Relative), new { action = "print" }),
            "edit" => await actor.PutAsync(new Uri($"/api/files/{id}/content", UriKind.Relative), new ByteArrayContent([4, 5])),
            "rename" => await actor.PatchAsJsonAsync(new Uri($"/api/files/{id}", UriKind.Relative), new { name = "mới.docx" }),
            "restore" => await actor.PostAsync(new Uri($"/api/files/{id}/versions/1/restore", UriKind.Relative), null),
            "shares" => await actor.GetAsync(new Uri($"/api/files/{id}/shares", UriKind.Relative)),
            "audit" => await actor.GetAsync(new Uri($"/api/files/{id}/audit", UriKind.Relative)),
            "delete" => await actor.DeleteAsync(new Uri($"/api/files/{id}", UriKind.Relative)),
            _ => throw new ArgumentOutOfRangeException(nameof(op)),
        };
        Assert.Equal(expected, res.StatusCode);
    }

    [Fact]
    public async Task FolderShareIsInherited()
    {
        var owner = fx.App.ClientFor("fo-owner", "Owner");
        var editor = fx.App.ClientFor("fo-editor", "Editor");
        await editor.GetAsync(new Uri("/api/me", UriKind.Relative));
        var folder = await (await owner.PostAsJsonAsync(new Uri("/api/folders", UriKind.Relative), new { name = "Hợp đồng" })).Content.ReadFromJsonAsync<JsonElement>();
        var sub = await (await owner.PostAsJsonAsync(new Uri("/api/folders", UriKind.Relative), new { name = "2026", parentId = folder.Id() })).Content.ReadFromJsonAsync<JsonElement>();
        var file = await Api.UploadAsync(owner, "hd.docx", [1], sub.Id());
        Assert.Equal(HttpStatusCode.NotFound, (await editor.GetAsync(new Uri($"/api/files/{file.Id()}", UriKind.Relative))).StatusCode);

        Assert.Equal(HttpStatusCode.OK, await Api.ShareAsync(owner, "folders", folder.Id(), "fo-editor", "editor"));
        Assert.Equal(HttpStatusCode.OK, (await editor.PutAsync(new Uri($"/api/files/{file.Id()}/content", UriKind.Relative), new ByteArrayContent([2]))).StatusCode);
        await Api.UploadAsync(editor, "new.docx", [3], sub.Id());
        var listing = await editor.GetFromJsonAsync<JsonElement>(new Uri($"/api/folders/{sub.Id()}", UriKind.Relative));
        Assert.Equal(2, listing.GetProperty("files").GetArrayLength());
        var shared = await editor.GetFromJsonAsync<JsonElement>(new Uri("/api/shared", UriKind.Relative));
        Assert.Equal("Hợp đồng", shared.GetProperty("folders")[0].GetProperty("name").GetString());

        // Deleting the parent folder hides everything below it.
        Assert.Equal(HttpStatusCode.NoContent, (await owner.DeleteAsync(new Uri($"/api/folders/{folder.Id()}", UriKind.Relative))).StatusCode);
        Assert.Equal(HttpStatusCode.NotFound, (await editor.GetAsync(new Uri($"/api/files/{file.Id()}", UriKind.Relative))).StatusCode);
    }

    [Fact]
    public async Task CannotMoveFolderIntoItself()
    {
        var owner = fx.App.ClientFor("mv-owner");
        var a = await (await owner.PostAsJsonAsync(new Uri("/api/folders", UriKind.Relative), new { name = "A" })).Content.ReadFromJsonAsync<JsonElement>();
        var b = await (await owner.PostAsJsonAsync(new Uri("/api/folders", UriKind.Relative), new { name = "B", parentId = a.Id() })).Content.ReadFromJsonAsync<JsonElement>();
        var res = await owner.PatchAsJsonAsync(new Uri($"/api/folders/{a.Id()}", UriKind.Relative), new { parentId = b.Id() });
        Assert.Equal(HttpStatusCode.BadRequest, res.StatusCode);
    }

    [Fact]
    public void EvaluatorIgnoresExpiredShares()
    {
        var user = Guid.NewGuid();
        var now = DateTime.UtcNow;
        var shares = new[]
        {
            new Share { GranteeUserId = user, Role = ShareRole.Editor, ExpiresAt = now.AddMinutes(-1) },
            new Share { GranteeUserId = user, Role = ShareRole.Viewer },
        };
        Assert.Equal(ShareRole.Viewer, AccessEvaluator.EffectiveRole(user, [Guid.NewGuid()], shares, now));
        Assert.Equal(ShareRole.Owner, AccessEvaluator.EffectiveRole(user, [user], shares, now));
        Assert.False(AccessEvaluator.Allows(ShareRole.Commenter, Operation.Edit));
        Assert.True(AccessEvaluator.Allows(ShareRole.Commenter, Operation.Comment));
    }
}

public sealed class FileLifecycleTests(AppFixture fx) : IClassFixture<AppFixture>
{
    [Fact]
    public async Task UploadDownloadVersionsAndRestore()
    {
        var c = fx.App.ClientFor("lc-user", "Lan");
        var v1 = "phiên bản 1"u8.ToArray();
        var file = await Api.UploadAsync(c, "Tờ trình.docx", v1);
        var id = file.Id();
        Assert.Equal(1, file.GetProperty("version").GetInt32());
        Assert.Equal("owner", file.GetProperty("role").GetString());
        Assert.Equal("application/vnd.openxmlformats-officedocument.wordprocessingml.document", file.GetProperty("contentType").GetString());

        Assert.Equal(HttpStatusCode.OK, (await PutIfMatchAsync(c, id, "phiên bản 2"u8.ToArray(), "\"1\"")).StatusCode);

        // Stale If-Match → 409.
        Assert.Equal(HttpStatusCode.Conflict, (await PutIfMatchAsync(c, id, [9], "\"1\"")).StatusCode);

        var versions = await c.GetFromJsonAsync<JsonElement>(new Uri($"/api/files/{id}/versions", UriKind.Relative));
        Assert.Equal(2, versions.GetArrayLength());
        Assert.Equal(Convert.ToHexString(SHA256.HashData(v1)).ToLowerInvariant(), versions[1].GetProperty("sha256").GetString());

        var restored = await (await c.PostAsync(new Uri($"/api/files/{id}/versions/1/restore", UriKind.Relative), null)).Content.ReadFromJsonAsync<JsonElement>();
        Assert.Equal(3, restored.GetProperty("version").GetInt32());
        var res = await c.GetAsync(new Uri($"/api/files/{id}/content", UriKind.Relative));
        Assert.Equal(v1, await res.Content.ReadAsByteArrayAsync());
        Assert.Equal("\"3\"", res.Headers.ETag?.Tag);
        var old = await c.GetByteArrayAsync(new Uri($"/api/files/{id}/content?version=2", UriKind.Relative));
        Assert.Equal("phiên bản 2"u8.ToArray(), old);

        versions = await c.GetFromJsonAsync<JsonElement>(new Uri($"/api/files/{id}/versions", UriKind.Relative));
        Assert.Equal("Restored from version 1", versions[0].GetProperty("note").GetString());
        Assert.True(versions[0].GetProperty("current").GetBoolean());
    }

    private static Task<HttpResponseMessage> PutIfMatchAsync(HttpClient c, Guid id, byte[] body, string etag)
    {
        var req = new HttpRequestMessage(HttpMethod.Put, new Uri($"/api/files/{id}/content", UriKind.Relative)) { Content = new ByteArrayContent(body) };
        req.Headers.IfMatch.Add(new System.Net.Http.Headers.EntityTagHeaderValue(etag));
        return c.SendAsync(req);
    }

    [Fact]
    public async Task RejectsBadNames()
    {
        var c = fx.App.ClientFor("names-user");
        foreach (var bad in new[] { "../x.docx", "a/b.docx", "con:x", " " })
        {
            var res = await c.PostAsync(new Uri($"/api/files?name={Uri.EscapeDataString(bad)}", UriKind.Relative), new ByteArrayContent([1]));
            Assert.Equal(HttpStatusCode.BadRequest, res.StatusCode);
        }
    }

    [Fact]
    public async Task UnauthenticatedIsRejected()
    {
        var res = await fx.App.CreateClient().GetAsync(new Uri("/api/folders", UriKind.Relative));
        Assert.Equal(HttpStatusCode.Unauthorized, res.StatusCode);
    }
}

public sealed class UploadLimitTests : IDisposable
{
    private readonly TestApp _app = new(settings: new() { ["Storage:MaxUploadBytes"] = "1024" });

    public void Dispose() => _app.Dispose();

    [Fact]
    public async Task OversizedUploadIsRejectedAndLeavesNoFile()
    {
        var c = _app.ClientFor("big");
        var res = await c.PostAsync(new Uri("/api/files?name=big.docx", UriKind.Relative), new ByteArrayContent(new byte[4096]));
        Assert.Equal(HttpStatusCode.RequestEntityTooLarge, res.StatusCode);
        var root = await c.GetFromJsonAsync<JsonElement>(new Uri("/api/folders", UriKind.Relative));
        Assert.Equal(0, root.GetProperty("files").GetArrayLength());
        Assert.Empty(Directory.EnumerateFiles(Path.Combine(_app.ObjectsRoot, ".tmp")));
    }
}

public sealed class ShareLinkTests : IDisposable
{
    private readonly TestClock _clock = new();
    private readonly TestApp _app;

    public ShareLinkTests() => _app = new TestApp(clock: _clock);

    public void Dispose() => _app.Dispose();

    [Fact]
    public async Task LinkWorksAnonymouslyUntilItExpires()
    {
        var owner = _app.ClientFor("link-owner", "Owner");
        var file = await Api.UploadAsync(owner, "công văn.docx", "nội dung"u8.ToArray());
        var link = await (await owner.PostAsJsonAsync(new Uri($"/api/files/{file.Id()}/links", UriKind.Relative),
            new { role = "viewer", expiresAt = _clock.GetUtcNow().UtcDateTime.AddHours(1) })).Content.ReadFromJsonAsync<JsonElement>();
        var token = link.GetProperty("linkToken").GetString()!;
        Assert.True(token.Length >= 40);

        var anon = _app.CreateClient();
        var meta = await anon.GetFromJsonAsync<JsonElement>(new Uri($"/api/links/{token}", UriKind.Relative));
        Assert.Equal("công văn.docx", meta.GetProperty("file").GetProperty("name").GetString());
        Assert.Equal("nội dung"u8.ToArray(), await anon.GetByteArrayAsync(new Uri($"/api/links/{token}/content", UriKind.Relative)));

        _clock.Advance(TimeSpan.FromHours(2));
        Assert.Equal(HttpStatusCode.Gone, (await anon.GetAsync(new Uri($"/api/links/{token}", UriKind.Relative))).StatusCode);
        Assert.Equal(HttpStatusCode.Gone, (await anon.GetAsync(new Uri($"/api/links/{token}/content", UriKind.Relative))).StatusCode);
        Assert.Equal(HttpStatusCode.NotFound, (await anon.GetAsync(new Uri("/api/links/does-not-exist", UriKind.Relative))).StatusCode);
    }

    [Fact]
    public async Task ExpiringUserShareStopsGrantingAccess()
    {
        var owner = _app.ClientFor("exp-owner");
        var other = _app.ClientFor("exp-other");
        await other.GetAsync(new Uri("/api/me", UriKind.Relative));
        var file = await Api.UploadAsync(owner, "x.docx", [1]);
        Assert.Equal(HttpStatusCode.OK, await Api.ShareAsync(owner, "files", file.Id(), "exp-other", "viewer"));
        Assert.Equal(HttpStatusCode.OK, (await other.GetAsync(new Uri($"/api/files/{file.Id()}", UriKind.Relative))).StatusCode);

        var past = await owner.PostAsJsonAsync(new Uri($"/api/files/{file.Id()}/links", UriKind.Relative),
            new { role = "viewer", expiresAt = _clock.GetUtcNow().UtcDateTime.AddMinutes(-5) });
        Assert.Equal(HttpStatusCode.BadRequest, past.StatusCode);
    }

    [Fact]
    public async Task RemovedLinkIsGone()
    {
        var owner = _app.ClientFor("rm-owner");
        var file = await Api.UploadAsync(owner, "x.docx", [1]);
        var link = await (await owner.PostAsJsonAsync(new Uri($"/api/files/{file.Id()}/links", UriKind.Relative), new { role = "viewer" }))
            .Content.ReadFromJsonAsync<JsonElement>();
        var shares = await owner.GetFromJsonAsync<JsonElement>(new Uri($"/api/files/{file.Id()}/shares", UriKind.Relative));
        Assert.Equal(1, shares.GetArrayLength());
        Assert.Equal(HttpStatusCode.NoContent, (await owner.DeleteAsync(new Uri($"/api/shares/{link.Id()}", UriKind.Relative))).StatusCode);
        Assert.Equal(HttpStatusCode.NotFound, (await _app.CreateClient().GetAsync(new Uri($"/api/links/{link.GetProperty("linkToken").GetString()}", UriKind.Relative))).StatusCode);
    }
}

public sealed class AuditTests(AppFixture fx) : IClassFixture<AppFixture>
{
    [Fact]
    public async Task DownloadsPrintsAndEditsAreAudited()
    {
        var owner = fx.App.ClientFor("audit-owner", "Phạm Thu");
        var file = await Api.UploadAsync(owner, "audit.docx", [1]);
        var id = file.Id();
        await owner.GetByteArrayAsync(new Uri($"/api/files/{id}/content", UriKind.Relative));
        await owner.PutAsync(new Uri($"/api/files/{id}/content", UriKind.Relative), new ByteArrayContent([2]));
        Assert.Equal(HttpStatusCode.NoContent, (await owner.PostAsJsonAsync(new Uri($"/api/files/{id}/audit", UriKind.Relative), new { action = "print", detail = "2 bản" })).StatusCode);
        Assert.Equal(HttpStatusCode.NoContent, (await owner.PostAsJsonAsync(new Uri($"/api/files/{id}/audit", UriKind.Relative), new { action = "export", detail = "pdf" })).StatusCode);
        Assert.Equal(HttpStatusCode.BadRequest, (await owner.PostAsJsonAsync(new Uri($"/api/files/{id}/audit", UriKind.Relative), new { action = "delete" })).StatusCode);

        var audit = await owner.GetFromJsonAsync<JsonElement>(new Uri($"/api/files/{id}/audit", UriKind.Relative));
        var actions = audit.EnumerateArray().Select(a => a.GetProperty("action").GetString()).ToList();
        Assert.Equal(["export", "print", "edit", "download", "upload"], actions);
        Assert.All(audit.EnumerateArray(), a => Assert.Equal("Phạm Thu", a.GetProperty("actorName").GetString()));
    }
}
