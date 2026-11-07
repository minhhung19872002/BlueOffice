using System.Net;
using System.Net.Http.Json;
using System.Text;
using System.Text.Json;
using MockBlueDrive;
using BlueOffice.Api.Sessions;
using Xunit;

namespace BlueOffice.Tests;

/// <summary>
/// Phase 6A acceptance scenarios against <c>tests/mock-bluedrive</c> (BOHP + JWKS + webhooks): open a file, two users in one
/// session, save → version with editors, permission revoked mid-session, offline edit → conflict copy.
/// </summary>
public sealed class HostIntegrationTests : IAsyncLifetime
{
    private readonly TestClock _clock = new();
    private MockDrive _drive = null!;
    private TestApp _app = null!;

    public async Task InitializeAsync()
    {
        _drive = await MockDrive.StartAsync();
        _app = new TestApp(_drive, clock: _clock);
    }

    public async Task DisposeAsync()
    {
        _app.Dispose();
        await _drive.DisposeAsync();
    }

    private async Task<JsonElement> OpenAsync(string user, string fileId)
    {
        var token = await _drive.TokenAsync(user, fileId);
        var res = await _app.CreateClient().PostAsJsonAsync(new Uri("/api/sessions", UriKind.Relative), new { host = "bluedrive", fileId, token });
        Assert.Equal(HttpStatusCode.Created, res.StatusCode);
        return await res.Content.ReadFromJsonAsync<JsonElement>();
    }

    private Task<WsClient> ConnectAsync(JsonElement session) =>
        _app.ConnectAsync($"{session.GetProperty("collab").GetProperty("url").GetString()}&token={session.GetProperty("sessionToken").GetString()}");

    private HttpClient SessionClient(JsonElement session)
    {
        var c = _app.CreateClient();
        c.DefaultRequestHeaders.Add(SessionEndpoints.TokenHeader, session.GetProperty("sessionToken").GetString());
        return c;
    }

    private static string Sid(JsonElement session) => session.GetProperty("sessionId").GetString()!;

    private async Task<string> SharedFileAsync(string name = "Báo cáo quý.docx", string content = "v1")
    {
        var id = await _drive.CreateFileAsync(name, content);
        await _drive.SetPermsAsync(id, "u2", view: true, edit: true);
        await _drive.SetPermsAsync(id, "u3", view: true, edit: false);
        return id;
    }

    private async Task<WebhookRecord> WebhookAsync(string evt, string fileId, Func<WebhookRecord, bool>? match = null)
    {
        WebhookRecord? found = null;
        await CollabTests.Eventually(() =>
        {
            found = _drive.WebhooksAsync().GetAwaiter().GetResult()
                .LastOrDefault(w => w.Event == evt && w.FileId == fileId && (match is null || match(w)));
            return found is not null;
        });
        Assert.True(found!.SignatureValid, "webhook signature");
        return found;
    }

    [Fact]
    public async Task OpensFileFromBlueDriveThroughTheEditUrl()
    {
        var id = await SharedFileAsync(content: "nội dung gốc");
        var token = await _drive.TokenAsync("u2", id);
        var anon = _app.CreateClient();
        var info = await anon.GetFromJsonAsync<JsonElement>(new Uri($"/edit?file={id}&host=bluedrive&token={token}", UriKind.Relative));
        Assert.Equal("bluedrive", info.GetProperty("host").GetString());
        Assert.Equal("bohp", info.GetProperty("protocol").GetString());
        Assert.Equal("edit", info.GetProperty("mode").GetString());
        Assert.Equal("Báo cáo quý.docx", info.GetProperty("file").GetProperty("name").GetString());
        Assert.Equal("1", info.GetProperty("file").GetProperty("version").GetString());
        Assert.Equal("Trần Thị Bình", info.GetProperty("user").GetProperty("name").GetString());
        Assert.Equal("https://drive.mock", info.GetProperty("allowedOrigins")[0].GetString());

        var content = await SessionClient(info).GetAsync(new Uri($"/api/sessions/{Sid(info)}/contents", UriKind.Relative));
        Assert.Equal("nội dung gốc", await content.Content.ReadAsStringAsync());
        Assert.Equal("1", content.Headers.GetValues("X-Version").Single());

        // Opening is audited and forwarded to the host.
        var audit = await WebhookAsync("audit", id);
        Assert.Contains("\"action\":\"open\"", audit.Body, StringComparison.Ordinal);

        // Viewer gets view mode; a token for another file, a forged token and a user without access are rejected.
        Assert.Equal("view", (await OpenAsync("u3", id)).GetProperty("mode").GetString());
        var other = await _drive.TokenAsync("u2", "f1");
        Assert.Equal(HttpStatusCode.Forbidden, (await anon.GetAsync(new Uri($"/edit?host=bluedrive&file={id}&token={other}", UriKind.Relative))).StatusCode);
        Assert.Equal(HttpStatusCode.Unauthorized, (await anon.GetAsync(new Uri($"/edit?host=bluedrive&file={id}&token={token}x", UriKind.Relative))).StatusCode);
        var stranger = await _drive.TokenAsync("intruder", id);
        Assert.Equal(HttpStatusCode.Forbidden, (await anon.GetAsync(new Uri($"/edit?host=bluedrive&file={id}&token={stranger}", UriKind.Relative))).StatusCode);
        Assert.Equal(HttpStatusCode.NotFound, (await anon.GetAsync(new Uri($"/edit?host=unknown&file={id}&token={token}", UriKind.Relative))).StatusCode);
    }

    [Fact]
    public async Task TwoUsersShareOneSessionLockAndSaveCreatesVersionWithEditors()
    {
        var id = await SharedFileAsync();
        var s1 = await OpenAsync("u1", id);
        var s2 = await OpenAsync("u2", id);
        await using var a = await ConnectAsync(s1);
        Assert.True((await a.WaitEventAsync("welcome")).GetProperty("seed").GetBoolean());
        await using var b = await ConnectAsync(s2);
        var welcome = await b.WaitEventAsync("welcome");
        Assert.False(welcome.GetProperty("seed").GetBoolean());
        Assert.Equal("edit", welcome.GetProperty("mode").GetString());
        await b.WaitAsync(m => m.EventType == "collaborators-changed" && m.Json.GetProperty("collaborators").GetArrayLength() == 2, "2 collaborators");

        // One BOHP lock, owned by the session (room), not by a user.
        string? lockId = null;
        await CollabTests.Eventually(() =>
        {
            var st = _drive.FileStateAsync(id).GetAwaiter().GetResult();
            lockId = st.GetProperty("lockId").GetString();
            return lockId is not null;
        });

        // Both edit; updates are relayed.
        await a.SendAsync([0, 2, 1, 1]);
        await b.WaitBinaryAsync([0, 2, 1, 1]);
        await b.SendAsync([0, 2, 1, 2]);
        await a.WaitBinaryAsync([0, 2, 1, 2]);

        var res = await SessionClient(s2).PostAsync(new Uri($"/api/sessions/{Sid(s2)}/checkpoint?save=true", UriKind.Relative), new ByteArrayContent("v2-docx"u8.ToArray()));
        var body = await res.Content.ReadFromJsonAsync<JsonElement>();
        Assert.Equal("saved", body.GetProperty("outcome").GetString());
        Assert.Equal("2", body.GetProperty("version").GetString());
        var saved = await a.WaitEventAsync("saved");
        Assert.Equal(2, saved.GetProperty("editors").GetArrayLength());

        var state = await _drive.FileStateAsync(id);
        var v2 = state.GetProperty("versions")[1];
        Assert.Equal("v2-docx", Encoding.UTF8.GetString(Convert.FromBase64String(v2.GetProperty("content").GetString()!)));
        Assert.Equal<string>(["u1", "u2"], v2.GetProperty("editors").EnumerateArray().Select(e => e.GetString()!).Order().ToArray());
        Assert.Contains("Nguyễn Văn An", v2.GetProperty("note").GetString(), StringComparison.Ordinal);
        Assert.Contains("Trần Thị Bình", v2.GetProperty("note").GetString(), StringComparison.Ordinal);
        Assert.Equal(lockId, state.GetProperty("lockId").GetString());

        var hook = await WebhookAsync("file.saved", id);
        Assert.Contains("\"version\":\"2\"", hook.Body, StringComparison.Ordinal);
        await WebhookAsync("session.started", id);

        // Last participant leaves → lock released, session.ended.
        await a.DisposeAsync();
        await b.DisposeAsync();
        await CollabTests.Eventually(() => _drive.FileStateAsync(id).GetAwaiter().GetResult().GetProperty("lockId").ValueKind == JsonValueKind.Null);
        var ended = await WebhookAsync("session.ended", id);
        Assert.Contains("\"unsavedChanges\":false", ended.Body, StringComparison.Ordinal);
    }

    [Fact]
    public async Task UnsavedCheckpointIsSavedWhenTheLastParticipantLeaves()
    {
        var id = await SharedFileAsync();
        var s1 = await OpenAsync("u1", id);
        await using (var a = await ConnectAsync(s1))
        {
            await a.WaitEventAsync("welcome");
            await a.SendAsync([0, 2, 1, 7]);
            await a.SendJsonAsync(new { type = "ping" });
            await a.WaitEventAsync("pong");
            var r = await SessionClient(s1).PostAsync(new Uri($"/api/sessions/{Sid(s1)}/checkpoint", UriKind.Relative), new ByteArrayContent("final"u8.ToArray()));
            Assert.Equal("upToDate", (await r.Content.ReadFromJsonAsync<JsonElement>()).GetProperty("outcome").GetString());
        }

        await CollabTests.Eventually(() => _drive.FileStateAsync(id).GetAwaiter().GetResult().GetProperty("version").GetString() == "2");
        var hook = await WebhookAsync("file.saved", id);
        Assert.Contains("session-end", hook.Body, StringComparison.Ordinal);
    }

    [Fact]
    public async Task AutosaveAsksForACheckpointAndSavesAfterTheInterval()
    {
        var id = await SharedFileAsync();
        var s1 = await OpenAsync("u1", id);
        await using var a = await ConnectAsync(s1);
        await a.WaitEventAsync("welcome");
        await a.SendAsync([0, 2, 1, 9]);
        await a.SendJsonAsync(new { type = "ping" });
        await a.WaitEventAsync("pong");

        // No checkpoint yet: after the autosave interval the leader is asked for one.
        _clock.Advance(TimeSpan.FromSeconds(46));
        var request = await a.WaitEventAsync("save-request");
        Assert.Equal("autosave", request.GetProperty("reason").GetString());

        // The client answers with a checkpoint → saved.
        var r = await SessionClient(s1).PostAsync(new Uri($"/api/sessions/{Sid(s1)}/checkpoint", UriKind.Relative), new ByteArrayContent("auto"u8.ToArray()));
        Assert.Equal("saved", (await r.Content.ReadFromJsonAsync<JsonElement>()).GetProperty("outcome").GetString());
        Assert.Equal("autosave", (await a.WaitEventAsync("saved")).GetProperty("reason").GetString());
        Assert.Equal("2", (await _drive.FileStateAsync(id)).GetProperty("version").GetString());
    }

    [Fact]
    public async Task RevokingPermissionMidSessionSwitchesToViewThenCloses()
    {
        var id = await SharedFileAsync();
        var s2 = await OpenAsync("u2", id);
        await using var b = await ConnectAsync(s2);
        Assert.Equal("edit", (await b.WaitEventAsync("welcome")).GetProperty("mode").GetString());

        // BlueDrive revokes edit and notifies BlueOffice with a service token.
        await _drive.SetPermsAsync(id, "u2", view: true, edit: false);
        var service = _app.CreateClient();
        service.DefaultRequestHeaders.Authorization = new("Bearer", await _drive.TokenAsync("bluedrive-service"));
        var res = await service.PostAsJsonAsync(new Uri("/api/hosts/bluedrive/notify", UriKind.Relative), new { @event = "permissions-changed", fileId = id, userId = "u2" });
        Assert.Equal(HttpStatusCode.Accepted, res.StatusCode);
        var changed = await b.WaitEventAsync("mode-changed");
        Assert.Equal("view", changed.GetProperty("mode").GetString());

        // Read-only now: updates are dropped and checkpoints refused.
        var cp = await SessionClient(s2).PostAsync(new Uri($"/api/sessions/{Sid(s2)}/checkpoint?save=true", UriKind.Relative), new ByteArrayContent([1]));
        Assert.Equal(HttpStatusCode.Forbidden, cp.StatusCode);

        // A user token cannot impersonate the host; another host name is refused.
        var user = _app.ClientFor("alice");
        Assert.NotEqual(HttpStatusCode.Accepted, (await user.PostAsJsonAsync(new Uri("/api/hosts/bluedrive/notify", UriKind.Relative), new { @event = "permissions-changed", fileId = id })).StatusCode);
        Assert.Equal(HttpStatusCode.Forbidden, (await service.PostAsJsonAsync(new Uri("/api/hosts/wopihost/notify", UriKind.Relative), new { @event = "permissions-changed", fileId = id })).StatusCode);

        // Losing view access closes the connection (refresh path, as triggered by a token refresh).
        await _drive.SetPermsAsync(id, "u2", view: false, edit: false);
        var refreshed = await SessionClient(s2).PostAsJsonAsync(new Uri($"/api/sessions/{Sid(s2)}/refresh", UriKind.Relative), new { });
        Assert.Equal("none", (await refreshed.Content.ReadFromJsonAsync<JsonElement>()).GetProperty("mode").GetString());
        await b.WaitEventAsync("session-revoked");
        await b.WaitClosedAsync();
        Assert.Equal(4403, (int?)b.CloseStatus);
        Assert.Equal(HttpStatusCode.Unauthorized, (await SessionClient(s2).GetAsync(new Uri($"/api/sessions/{Sid(s2)}", UriKind.Relative))).StatusCode);
    }

    [Fact]
    public async Task OfflineEditOnStaleVersionCreatesConflictCopy()
    {
        var id = await SharedFileAsync("Báo cáo quý.docx", "v1");
        var s2 = await OpenAsync("u2", id);
        Assert.Equal("1", s2.GetProperty("file").GetProperty("version").GetString());

        // While u2 is offline, someone uploads a new version in BlueDrive.
        (await _drive.Client.PutAsync($"/_admin/files/{id}/contents", new ByteArrayContent("external v2"u8.ToArray()))).EnsureSuccessStatusCode();

        var res = await SessionClient(s2).PostAsync(new Uri($"/api/sessions/{Sid(s2)}/sync?baseVersion=1", UriKind.Relative), new ByteArrayContent("offline edits"u8.ToArray()));
        Assert.Equal(HttpStatusCode.OK, res.StatusCode);
        var r = await res.Content.ReadFromJsonAsync<JsonElement>();
        Assert.Equal("conflict-copy", r.GetProperty("outcome").GetString());
        Assert.Equal("2", r.GetProperty("version").GetString());
        var copyName = r.GetProperty("conflictCopy").GetProperty("name").GetString()!;
        Assert.StartsWith("Báo cáo quý (xung đột - Trần Thị Bình - ", copyName, StringComparison.Ordinal);
        Assert.EndsWith(").docx", copyName, StringComparison.Ordinal);

        // Nothing was overwritten; the copy sits next to the original with the offline content.
        var original = await _drive.FileStateAsync(id);
        Assert.Equal("2", original.GetProperty("version").GetString());
        var listing = await _drive.Client.GetFromJsonAsync<JsonElement>("/_admin/folders/root/files");
        var copy = listing.EnumerateArray().Single(f => f.GetProperty("name").GetString() == copyName);
        var copyState = await _drive.FileStateAsync(copy.GetProperty("id").GetString()!);
        Assert.Equal("offline edits", Encoding.UTF8.GetString(Convert.FromBase64String(copyState.GetProperty("versions")[0].GetProperty("content").GetString()!)));
        Assert.Equal("u2", copy.GetProperty("ownerId").GetString());
        await WebhookAsync("file.conflict-copy", id);

        // Syncing against the current version simply saves.
        var ok = await (await SessionClient(s2).PostAsync(new Uri($"/api/sessions/{Sid(s2)}/sync?baseVersion=2", UriKind.Relative), new ByteArrayContent("rebased"u8.ToArray())))
            .Content.ReadFromJsonAsync<JsonElement>();
        Assert.Equal("saved", ok.GetProperty("outcome").GetString());
        Assert.Equal("3", ok.GetProperty("version").GetString());
        Assert.Equal(JsonValueKind.Null, ok.GetProperty("conflictCopy").ValueKind);
    }

    [Fact]
    public async Task OfflineSyncDoesNotOverwriteUnsavedCollaborativeChanges()
    {
        var id = await SharedFileAsync();
        var s1 = await OpenAsync("u1", id);
        var s2 = await OpenAsync("u2", id);
        await using var a = await ConnectAsync(s1);
        await a.WaitEventAsync("welcome");
        await a.SendAsync([0, 2, 1, 3]);
        await a.SendJsonAsync(new { type = "ping" });
        await a.WaitEventAsync("pong");

        var r = await (await SessionClient(s2).PostAsync(new Uri($"/api/sessions/{Sid(s2)}/sync?baseVersion=1", UriKind.Relative), new ByteArrayContent("offline"u8.ToArray())))
            .Content.ReadFromJsonAsync<JsonElement>();
        Assert.Equal("conflict-copy", r.GetProperty("outcome").GetString());
        Assert.Equal("1", (await _drive.FileStateAsync(id)).GetProperty("version").GetString());
    }

    [Fact]
    public async Task SessionVersionsRestoreAndRenameGoThroughBohp()
    {
        var id = await SharedFileAsync(content: "a");
        var s1 = await OpenAsync("u1", id);
        var c = SessionClient(s1);
        await c.PostAsync(new Uri($"/api/sessions/{Sid(s1)}/checkpoint?save=true", UriKind.Relative), new ByteArrayContent("b"u8.ToArray()));
        var versions = await c.GetFromJsonAsync<JsonElement>(new Uri($"/api/sessions/{Sid(s1)}/versions", UriKind.Relative));
        Assert.Equal<string>(["2", "1"], versions.GetProperty("versions").EnumerateArray().Select(v => v.GetProperty("version").GetString()!).ToArray());

        var restored = await (await c.PostAsync(new Uri($"/api/sessions/{Sid(s1)}/versions/1/restore", UriKind.Relative), null)).Content.ReadFromJsonAsync<JsonElement>();
        Assert.Equal("3", restored.GetProperty("version").GetString());
        Assert.Equal("a", Encoding.UTF8.GetString(await c.GetByteArrayAsync(new Uri($"/api/sessions/{Sid(s1)}/contents", UriKind.Relative))));

        var renamed = await (await c.PostAsJsonAsync(new Uri($"/api/sessions/{Sid(s1)}/rename", UriKind.Relative), new { name = "Báo cáo năm.docx" })).Content.ReadFromJsonAsync<JsonElement>();
        Assert.Equal("Báo cáo năm.docx", renamed.GetProperty("name").GetString());
        Assert.Equal("Báo cáo năm.docx", (await _drive.FileStateAsync(id)).GetProperty("name").GetString());

        var saveAs = await c.PostAsync(new Uri($"/api/sessions/{Sid(s1)}/save-as?name={Uri.EscapeDataString("Bản sao.docx")}&folderId=root", UriKind.Relative), new ByteArrayContent("copy"u8.ToArray()));
        Assert.Equal(HttpStatusCode.OK, saveAs.StatusCode);
        var listing = await _drive.Client.GetFromJsonAsync<JsonElement>("/_admin/folders/root/files");
        Assert.Contains(listing.EnumerateArray(), f => f.GetProperty("name").GetString() == "Bản sao.docx");

        // Viewer cannot restore.
        var s3 = await OpenAsync("u3", id);
        Assert.Equal(HttpStatusCode.Forbidden, (await SessionClient(s3).PostAsync(new Uri($"/api/sessions/{Sid(s3)}/versions/1/restore", UriKind.Relative), null)).StatusCode);
    }

    [Fact]
    public async Task LockHeldByAnotherClientMakesEveryoneReadOnly()
    {
        var id = await SharedFileAsync();

        // Some other editor (outside BlueOffice) holds the BOHP lock.
        var foreign = await _drive.TokenAsync("u1", id);
        using var lockReq = new HttpRequestMessage(HttpMethod.Post, $"/bohp/v1/files/{id}/lock") { Content = JsonContent.Create(new { lockId = "foreign", expiresInSeconds = 600 }) };
        lockReq.Headers.Authorization = new("Bearer", foreign);
        (await _drive.Client.SendAsync(lockReq)).EnsureSuccessStatusCode();

        var s2 = await OpenAsync("u2", id);
        await using var b = await ConnectAsync(s2);
        await b.WaitEventAsync("welcome");
        var changed = await b.WaitEventAsync("mode-changed");
        Assert.Equal("view", changed.GetProperty("mode").GetString());
        Assert.Equal("locked", changed.GetProperty("reason").GetString());
    }
}

public sealed class ConflictCopyNameTests
{
    [Fact]
    public void UsesLocalTimeAndKeepsExtension()
    {
        var at = new DateTimeOffset(2026, 11, 7, 7, 30, 0, TimeSpan.Zero);
        Assert.Equal("Báo cáo (xung đột - Trần Thị Bình - 2026-11-07 14-30).docx",
            SessionService.ConflictCopyName("Báo cáo.docx", "Trần Thị Bình", at, "Asia/Ho_Chi_Minh"));
        Assert.Equal("x (xung đột - ab - 2026-11-07 07-30).xlsx", SessionService.ConflictCopyName("x.xlsx", "a/b", at, "No/Such_Zone"));
    }

    [Fact]
    public void StaysWithinNameLimit()
    {
        var name = SessionService.ConflictCopyName(new string('a', 300) + ".pptx", "U", DateTimeOffset.UnixEpoch, "UTC");
        Assert.True(name.Length <= 255);
        Assert.EndsWith(" (xung đột - U - 1970-01-01 00-00).pptx", name, StringComparison.Ordinal);
    }
}
