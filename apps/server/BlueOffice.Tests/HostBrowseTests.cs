using System.Net;
using System.Net.Http.Headers;
using System.Net.Http.Json;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using BlueOffice.Api.Sessions;
using Microsoft.IdentityModel.Tokens;
using MockBlueDrive;
using Xunit;

namespace BlueOffice.Tests;

/// <summary>
/// Desktop Start screen and sync-folder detection (prompt §6A.3) through the server proxy of the BOHP browsing endpoints
/// (docs/bohp.md §A2), plus webhook delivery, audit forwarding and lock refresh against the mock BlueDrive.
/// </summary>
public sealed class HostBrowseTests : IAsyncLifetime
{
    private readonly TestClock _clock = new();
    private MockDrive _drive = null!;
    private TestApp _app = null!;

    public async Task InitializeAsync()
    {
        _drive = await MockDrive.StartAsync();
        _app = new TestApp(_drive, clock: _clock, settings: new()
        {
            ["Hosts:0:DisplayName"] = "BlueDrive",
            ["Hosts:0:SignIn:AuthorizeUrl"] = "http://mock/oauth/authorize",
            ["Hosts:0:SignIn:TokenUrl"] = "http://mock/oauth/token",
            ["Hosts:0:SignIn:ClientId"] = "blueoffice-desktop",
            ["Webhooks:RetryBaseDelay"] = "00:00:00.050",
        });
    }

    public async Task DisposeAsync()
    {
        _app.Dispose();
        await _drive.DisposeAsync();
    }

    private async Task<HttpClient> DriveClientAsync(string user, string? fileId = null)
    {
        var c = _app.CreateClient();
        c.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Bearer", await _drive.TokenAsync(user, fileId));
        return c;
    }

    private async Task<string> FolderAsync(string name, string parent = "root")
    {
        var r = await _drive.Client.PostAsJsonAsync("/_admin/folders", new CreateFolderRequest(name, parent, "u1"));
        r.EnsureSuccessStatusCode();
        return (await r.Content.ReadFromJsonAsync<JsonElement>()).GetProperty("id").GetString()!;
    }

    private async Task<string> FileAsync(string name, string folder, string owner = "u1")
    {
        var r = await _drive.Client.PostAsJsonAsync("/_admin/files", new CreateFileRequest(name, owner, Convert.ToBase64String("x"u8.ToArray()), folder));
        r.EnsureSuccessStatusCode();
        return (await r.Content.ReadFromJsonAsync<JsonElement>()).GetProperty("id").GetString()!;
    }

    private static List<string> Names(JsonElement items) => items.EnumerateArray().Select(i => i.GetProperty("name").GetString()!).ToList();

    [Fact]
    public async Task ListsHostsWithTheirSignInSettings()
    {
        var body = await _app.CreateClient().GetFromJsonAsync<JsonElement>(new Uri("/api/hosts", UriKind.Relative));
        var hosts = body.GetProperty("hosts").EnumerateArray().ToList();
        var drive = hosts.Single(h => h.GetProperty("name").GetString() == "bluedrive");
        Assert.Equal("BlueDrive", drive.GetProperty("displayName").GetString());
        Assert.True(drive.GetProperty("browse").GetBoolean());
        Assert.Equal("blueoffice-desktop", drive.GetProperty("signIn").GetProperty("clientId").GetString());
        Assert.Equal("openid profile", drive.GetProperty("signIn").GetProperty("scope").GetString());
        var wopi = hosts.Single(h => h.GetProperty("name").GetString() == "wopihost");
        Assert.False(wopi.GetProperty("browse").GetBoolean());
        Assert.Equal(JsonValueKind.Null, wopi.GetProperty("signIn").ValueKind);
    }

    [Fact]
    public async Task BrowsesFoldersRecentAndSharedFilesThroughTheServer()
    {
        var project = await FolderAsync("Dự án");
        var plan = await FileAsync("Kế hoạch.docx", project);
        await _drive.SetPermsAsync(plan, "u2", view: true, edit: true);
        var hidden = await FileAsync("Lương.xlsx", project);
        var own = await FileAsync("Ghi chú của Bình.docx", "root", owner: "u2");

        var u2 = await DriveClientAsync("u2");
        var me = await u2.GetFromJsonAsync<JsonElement>(new Uri("/api/hosts/bluedrive/me", UriKind.Relative));
        Assert.Equal("Trần Thị Bình", me.GetProperty("name").GetString());

        var root = await u2.GetFromJsonAsync<JsonElement>(new Uri("/api/hosts/bluedrive/folders/root/children", UriKind.Relative));
        Assert.Equal("root", root.GetProperty("folder").GetProperty("id").GetString());
        var rootItems = root.GetProperty("items");
        var folder = rootItems.EnumerateArray().First(i => i.GetProperty("kind").GetString() == "folder");
        Assert.Equal(project, folder.GetProperty("id").GetString());
        Assert.Contains("Ghi chú của Bình.docx", Names(rootItems));
        Assert.DoesNotContain("Báo cáo.docx", Names(rootItems)); // the mock seed file is not shared with u2

        var inside = await u2.GetFromJsonAsync<JsonElement>(new Uri($"/api/hosts/bluedrive/folders/{project}/children", UriKind.Relative));
        Assert.Equal("root", inside.GetProperty("folder").GetProperty("parentId").GetString());
        Assert.Equal<string>(["Kế hoạch.docx"], Names(inside.GetProperty("items"))); // no permission on Lương.xlsx
        var item = inside.GetProperty("items")[0];
        Assert.Equal("file", item.GetProperty("kind").GetString());
        Assert.Equal("1", item.GetProperty("version").GetString());

        var shared = await u2.GetFromJsonAsync<JsonElement>(new Uri("/api/hosts/bluedrive/shared", UriKind.Relative));
        Assert.Equal<string>(["Kế hoạch.docx"], Names(shared.GetProperty("items")));

        // Opening files (session + download) makes them "recent", newest first.
        foreach (var id in new[] { own, plan })
        {
            var s = await (await u2.PostAsJsonAsync(new Uri("/api/sessions", UriKind.Relative), new { host = "bluedrive", fileId = id, token = await _drive.TokenAsync("u2") }))
                .Content.ReadFromJsonAsync<JsonElement>();
            var c = _app.CreateClient();
            c.DefaultRequestHeaders.Add(SessionEndpoints.TokenHeader, s.GetProperty("sessionToken").GetString());
            (await c.GetAsync(new Uri($"/api/sessions/{s.GetProperty("sessionId").GetString()}/contents", UriKind.Relative))).EnsureSuccessStatusCode();
        }

        var recent = await u2.GetFromJsonAsync<JsonElement>(new Uri("/api/hosts/bluedrive/recent?limit=5", UriKind.Relative));
        Assert.Equal<string>(["Kế hoạch.docx", "Ghi chú của Bình.docx"], Names(recent.GetProperty("items")));
        Assert.NotEqual(JsonValueKind.Null, recent.GetProperty("items")[0].GetProperty("openedAt").ValueKind);
        Assert.NotNull(hidden);
    }

    [Fact]
    public async Task ResolvesSyncFolderPathsToFileIds()
    {
        var project = await FolderAsync("Dự án");
        var sub = await FolderAsync("Quý 4", project);
        var plan = await FileAsync("Kế hoạch.docx", sub);
        await _drive.SetPermsAsync(plan, "u2", view: true, edit: true);
        var u2 = await DriveClientAsync("u2");

        var found = await u2.GetFromJsonAsync<JsonElement>(new Uri("/api/hosts/bluedrive/resolve?path=" + Uri.EscapeDataString(@"Dự án\Quý 4\kế hoạch.docx"), UriKind.Relative));
        Assert.Equal(plan, found.GetProperty("id").GetString());
        Assert.Equal("Kế hoạch.docx", found.GetProperty("name").GetString());

        Assert.Equal(HttpStatusCode.NotFound, (await u2.GetAsync(new Uri("/api/hosts/bluedrive/resolve?path=" + Uri.EscapeDataString("Dự án/Khác.docx"), UriKind.Relative))).StatusCode);
        Assert.Equal(HttpStatusCode.BadRequest, (await u2.GetAsync(new Uri("/api/hosts/bluedrive/resolve?path=" + Uri.EscapeDataString("../x.docx"), UriKind.Relative))).StatusCode);
        Assert.Equal(HttpStatusCode.BadRequest, (await u2.GetAsync(new Uri("/api/hosts/bluedrive/resolve", UriKind.Relative))).StatusCode);

        // u3 has no permission on the file: not found (the host decides).
        var u3 = await DriveClientAsync("u3");
        Assert.Equal(HttpStatusCode.NotFound, (await u3.GetAsync(new Uri("/api/hosts/bluedrive/resolve?path=" + Uri.EscapeDataString("Dự án/Quý 4/Kế hoạch.docx"), UriKind.Relative))).StatusCode);
    }

    [Fact]
    public async Task BrowsingNeedsAValidUserTokenOfThatHost()
    {
        var anon = _app.CreateClient();
        Assert.Equal(HttpStatusCode.Unauthorized, (await anon.GetAsync(new Uri("/api/hosts/bluedrive/me", UriKind.Relative))).StatusCode);

        var forged = _app.CreateClient();
        forged.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Bearer", (await _drive.TokenAsync("u2")) + "x");
        Assert.Equal(HttpStatusCode.Unauthorized, (await forged.GetAsync(new Uri("/api/hosts/bluedrive/shared", UriKind.Relative))).StatusCode);

        // A BlueOffice user token is not a host token.
        Assert.Equal(HttpStatusCode.Unauthorized, (await _app.ClientFor("alice").GetAsync(new Uri("/api/hosts/bluedrive/me", UriKind.Relative))).StatusCode);

        // A token scoped to one file cannot list folders (the host refuses it).
        var scoped = await DriveClientAsync("u2", "f1");
        Assert.Equal(HttpStatusCode.Forbidden, (await scoped.GetAsync(new Uri("/api/hosts/bluedrive/folders/root/children", UriKind.Relative))).StatusCode);

        var u2 = await DriveClientAsync("u2");
        Assert.Equal(HttpStatusCode.NotFound, (await u2.GetAsync(new Uri("/api/hosts/nowhere/me", UriKind.Relative))).StatusCode);
        Assert.Equal(HttpStatusCode.NotImplemented, (await u2.GetAsync(new Uri("/api/hosts/wopihost/me", UriKind.Relative))).StatusCode);
        Assert.Equal(HttpStatusCode.NotFound, (await u2.GetAsync(new Uri("/api/hosts/bluedrive/folders/nope/children", UriKind.Relative))).StatusCode);
    }

    [Fact]
    public async Task MockSignInIssuesHostTokensWithPkce()
    {
        // RFC 7636: S256 challenge of a random verifier.
        var verifier = Base64UrlEncoder.Encode(RandomNumberGenerator.GetBytes(32));
        var challenge = Base64UrlEncoder.Encode(SHA256.HashData(Encoding.ASCII.GetBytes(verifier)));
        const string redirect = "bluedoc://auth";
        var authorize = $"/oauth/authorize?response_type=code&client_id=blueoffice-desktop&redirect_uri={Uri.EscapeDataString(redirect)}&state=s1&code_challenge={challenge}&code_challenge_method=S256&login_hint=u2";
        var res = await _drive.Client.GetAsync(authorize);
        Assert.Equal(HttpStatusCode.Redirect, res.StatusCode);
        var location = res.Headers.Location!.OriginalString;
        Assert.StartsWith("bluedoc://auth?code=", location, StringComparison.Ordinal);
        Assert.EndsWith("&state=s1", location, StringComparison.Ordinal);
        var code = Uri.UnescapeDataString(location.Split("code=")[1].Split('&')[0]);

        async Task<HttpResponseMessage> ExchangeAsync(string v) => await _drive.Client.PostAsync("/oauth/token", new FormUrlEncodedContent(new Dictionary<string, string>
        {
            ["grant_type"] = "authorization_code",
            ["code"] = code,
            ["code_verifier"] = v,
            ["redirect_uri"] = redirect,
            ["client_id"] = "blueoffice-desktop",
        }));

        var token = (await (await ExchangeAsync(verifier)).Content.ReadFromJsonAsync<JsonElement>()).GetProperty("access_token").GetString();
        var c = _app.CreateClient();
        c.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Bearer", token);
        Assert.Equal("u2", (await c.GetFromJsonAsync<JsonElement>(new Uri("/api/hosts/bluedrive/me", UriKind.Relative))).GetProperty("id").GetString());

        // Codes are single-use; a wrong verifier is refused.
        Assert.Equal(HttpStatusCode.BadRequest, (await ExchangeAsync(verifier)).StatusCode);
        var again = await _drive.Client.GetAsync(authorize);
        code = Uri.UnescapeDataString(again.Headers.Location!.OriginalString.Split("code=")[1].Split('&')[0]);
        Assert.Equal(HttpStatusCode.BadRequest, (await ExchangeAsync(verifier + "x")).StatusCode);

        // Without a hint the mock shows a sign-in page; a plain challenge is refused.
        Assert.Contains("login_hint=u2", await _drive.Client.GetStringAsync(authorize.Replace("&login_hint=u2", "", StringComparison.Ordinal)), StringComparison.Ordinal);
        Assert.Equal(HttpStatusCode.BadRequest, (await _drive.Client.GetAsync(authorize.Replace("S256", "plain", StringComparison.Ordinal))).StatusCode);
    }

    [Fact]
    public async Task WebhooksAreRetriedOnServerErrorsAndSigned()
    {
        (await _drive.Client.PostAsJsonAsync("/_admin/webhooks/fail", new FailRequest(2, 503))).EnsureSuccessStatusCode();
        var id = await FileAsync("Tờ trình.docx", "root");
        var u1 = await DriveClientAsync("u1");
        (await u1.PostAsJsonAsync(new Uri("/api/sessions", UriKind.Relative), new { host = "bluedrive", fileId = id, token = await _drive.TokenAsync("u1") })).EnsureSuccessStatusCode();

        WebhookRecord? hook = null;
        await CollabTests.Eventually(() => (hook = _drive.WebhooksAsync().GetAwaiter().GetResult().FirstOrDefault(w => w.FileId == id)) is not null);
        Assert.Equal("audit", hook!.Event);
        Assert.Equal(3, hook.Attempt); // two refused deliveries, then accepted
        Assert.True(hook.SignatureValid);
        using var body = JsonDocument.Parse(hook.Body);
        Assert.Equal("bluedrive", body.RootElement.GetProperty("host").GetString());
        Assert.Equal("open", body.RootElement.GetProperty("data").GetProperty("action").GetString());
        Assert.Equal("u1", body.RootElement.GetProperty("data").GetProperty("actor").GetString());
    }

    [Fact]
    public async Task WebhooksGiveUpAfterTheConfiguredAttemptsAndClientErrors()
    {
        (await _drive.Client.PostAsJsonAsync("/_admin/webhooks/fail", new FailRequest(3, 500))).EnsureSuccessStatusCode();
        var lost = await FileAsync("Mất.docx", "root");
        var u1 = await DriveClientAsync("u1");
        (await u1.PostAsJsonAsync(new Uri("/api/sessions", UriKind.Relative), new { host = "bluedrive", fileId = lost, token = await _drive.TokenAsync("u1") })).EnsureSuccessStatusCode();

        // A 4xx is final (no retry): one attempt, then the next event goes through.
        await CollabTests.Eventually(() => _drive.WebhooksAsync().GetAwaiter().GetResult().Count == 0 && AttemptsSoFar() >= 3);
        (await _drive.Client.PostAsJsonAsync("/_admin/webhooks/fail", new FailRequest(1, 400))).EnsureSuccessStatusCode();
        var refused = await FileAsync("Từ chối.docx", "root");
        (await u1.PostAsJsonAsync(new Uri("/api/sessions", UriKind.Relative), new { host = "bluedrive", fileId = refused, token = await _drive.TokenAsync("u1") })).EnsureSuccessStatusCode();
        var next = await FileAsync("Tiếp.docx", "root");
        (await u1.PostAsJsonAsync(new Uri("/api/sessions", UriKind.Relative), new { host = "bluedrive", fileId = next, token = await _drive.TokenAsync("u1") })).EnsureSuccessStatusCode();
        await CollabTests.Eventually(() => _drive.WebhooksAsync().GetAwaiter().GetResult().Any(w => w.FileId == next));

        var all = await _drive.WebhooksAsync();
        Assert.DoesNotContain(all, w => w.FileId == lost || w.FileId == refused);
        Assert.Equal(5, all.Single(w => w.FileId == next).Attempt); // 3 (lost) + 1 (refused) + 1

        int AttemptsSoFar() => _drive.App.Services.GetService(typeof(MockState)) is MockState s ? s.WebhookAttempts : 0;
    }

    [Fact]
    public async Task EditorAuditEventsAreForwardedToTheHost()
    {
        var id = await FileAsync("Hợp đồng.docx", "root");
        await _drive.SetPermsAsync(id, "u2", view: true, edit: false);
        var u1 = await DriveClientAsync("u1");
        async Task<HttpClient> SessionAsync(string user)
        {
            var s = await (await u1.PostAsJsonAsync(new Uri("/api/sessions", UriKind.Relative), new { host = "bluedrive", fileId = id, token = await _drive.TokenAsync(user) }))
                .Content.ReadFromJsonAsync<JsonElement>();
            var c = _app.CreateClient();
            c.DefaultRequestHeaders.Add(SessionEndpoints.TokenHeader, s.GetProperty("sessionToken").GetString());
            c.BaseAddress = new Uri(c.BaseAddress!, $"/api/sessions/{s.GetProperty("sessionId").GetString()}/");
            return c;
        }

        var owner = await SessionAsync("u1");
        foreach (var (action, detail) in new[] { ("print", "1 bản"), ("export", "pdf"), ("download", "docx") })
        {
            Assert.Equal(HttpStatusCode.NoContent, (await owner.PostAsJsonAsync(new Uri("audit", UriKind.Relative), new { action, detail })).StatusCode);
        }

        // Download permission is enforced for exports; other actions cannot be reported by the editor.
        await _drive.Client.PutAsJsonAsync($"/_admin/files/{id}/permissions/u2", new Perms(true, false, false, false, false));
        var viewer = await SessionAsync("u2");
        Assert.Equal(HttpStatusCode.Forbidden, (await viewer.PostAsJsonAsync(new Uri("audit", UriKind.Relative), new { action = "export", detail = "pdf" })).StatusCode);
        Assert.Equal(HttpStatusCode.NoContent, (await viewer.PostAsJsonAsync(new Uri("audit", UriKind.Relative), new { action = "print" })).StatusCode);
        Assert.Equal(HttpStatusCode.BadRequest, (await owner.PostAsJsonAsync(new Uri("audit", UriKind.Relative), new { action = "edit" })).StatusCode);

        List<(string Action, string Actor)> actions = [];
        await CollabTests.Eventually(() =>
        {
            actions = _drive.WebhooksAsync().GetAwaiter().GetResult()
                .Where(w => w.FileId == id && w.Event == "audit" && w.SignatureValid)
                .Select(w =>
                {
                    using var d = JsonDocument.Parse(w.Body);
                    var data = d.RootElement.GetProperty("data");
                    return (data.GetProperty("action").GetString()!, data.GetProperty("actor").GetString()!);
                })
                .ToList();
            return actions.Count >= 6;
        });
        Assert.Equal<(string Action, string Actor)>(
            [("open", "u1"), ("print", "u1"), ("export", "u1"), ("download", "u1"), ("open", "u2"), ("print", "u2")],
            actions);
    }

    [Fact]
    public async Task SessionLockIsRefreshedAndAutosaveOnlyRunsAfterChanges()
    {
        var id = await FileAsync("Biên bản.docx", "root");
        var u1 = await DriveClientAsync("u1");
        var s = await (await u1.PostAsJsonAsync(new Uri("/api/sessions", UriKind.Relative), new { host = "bluedrive", fileId = id, token = await _drive.TokenAsync("u1") }))
            .Content.ReadFromJsonAsync<JsonElement>();
        await using var ws = await _app.ConnectAsync($"{s.GetProperty("collab").GetProperty("url").GetString()}&token={s.GetProperty("sessionToken").GetString()}");
        await ws.WaitEventAsync("welcome");

        string? lockId = null;
        DateTime expires = default;
        await CollabTests.Eventually(() =>
        {
            var st = _drive.FileStateAsync(id).GetAwaiter().GetResult();
            lockId = st.GetProperty("lockId").GetString();
            if (lockId is not null)
            {
                expires = st.GetProperty("lockExpiresAt").GetDateTime();
            }

            return lockId is not null;
        });

        // Unchanged document: no save request even after the autosave interval.
        _clock.Advance(TimeSpan.FromSeconds(50));
        await ws.SendJsonAsync(new { type = "ping" });
        await ws.WaitEventAsync("pong");
        await Task.Delay(400);
        Assert.DoesNotContain(ws.Messages, m => m.EventType == "save-request");

        // Half the lock duration (1800 s) later the room refreshes the same lock.
        _clock.Advance(TimeSpan.FromSeconds(901));
        await CollabTests.Eventually(() =>
        {
            var st = _drive.FileStateAsync(id).GetAwaiter().GetResult();
            return st.GetProperty("lockId").GetString() == lockId && st.GetProperty("lockExpiresAt").GetDateTime() > expires;
        });
        Assert.Equal("1", (await _drive.FileStateAsync(id)).GetProperty("version").GetString());
    }
}
