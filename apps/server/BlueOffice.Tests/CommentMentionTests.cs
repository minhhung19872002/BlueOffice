using System.Net;
using System.Net.Http.Json;
using System.Text.Json;
using BlueOffice.Api.Sessions;
using BlueOffice.Domain;
using BlueOffice.Infrastructure.Mail;
using Microsoft.AspNetCore.Mvc.Testing;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.DependencyInjection.Extensions;
using Microsoft.Extensions.Logging.Abstractions;
using MockBlueDrive;
using Xunit;

namespace BlueOffice.Tests;

/// <summary>Records e-mails instead of sending them.</summary>
public sealed class FakeMailSender : IMailSender
{
    /// <summary>Sent e-mails.</summary>
    public List<OutgoingMail> Sent { get; } = [];

    /// <inheritdoc />
    public bool IsConfigured { get; set; } = true;

    /// <inheritdoc />
    public Task SendAsync(OutgoingMail mail, CancellationToken ct)
    {
        lock (Sent)
        {
            Sent.Add(mail);
        }

        return Task.CompletedTask;
    }
}

/// <summary>
/// CORE-COMMENT-001: the @mention people picker (<c>GET /api/sessions/{id}/people</c>, host directory via BOHP §A3 +
/// collaborators) and the notifications of a posted comment (<c>POST …/mentions</c> → BOHP <c>/notifications</c> + SMTP).
/// </summary>
public sealed class CommentMentionTests : IAsyncLifetime
{
    private MockDrive _drive = null!;
    private TestApp _app = null!;

    public async Task InitializeAsync()
    {
        _drive = await MockDrive.StartAsync();
        _app = new TestApp(_drive);
    }

    public async Task DisposeAsync()
    {
        _app.Dispose();
        await _drive.DisposeAsync();
    }

    private async Task<string> SharedFileAsync()
    {
        var id = await _drive.CreateFileAsync("Kế hoạch.docx", "v1");
        await _drive.SetPermsAsync(id, "u2", view: true, edit: true);
        await _drive.SetPermsAsync(id, "u3", view: true, edit: false);
        return id;
    }

    private static async Task<(HttpClient Client, string Sid)> OpenAsync(WebApplicationFactory<Program> app, MockDrive drive, string user, string fileId)
    {
        var token = await drive.TokenAsync(user, fileId);
        var res = await app.CreateClient().PostAsJsonAsync(new Uri("/api/sessions", UriKind.Relative), new { host = "bluedrive", fileId, token });
        Assert.Equal(HttpStatusCode.Created, res.StatusCode);
        var s = await res.Content.ReadFromJsonAsync<JsonElement>();
        var c = app.CreateClient();
        c.DefaultRequestHeaders.Add(SessionEndpoints.TokenHeader, s.GetProperty("sessionToken").GetString());
        return (c, s.GetProperty("sessionId").GetString()!);
    }

    private static async Task<List<JsonElement>> PeopleAsync(HttpClient c, string sid, string q) =>
        (await c.GetFromJsonAsync<JsonElement>(new Uri($"/api/sessions/{sid}/people?q={Uri.EscapeDataString(q)}", UriKind.Relative)))
            .GetProperty("people").EnumerateArray().ToList();

    [Fact]
    public async Task PeopleEndpointAsksTheHostDirectory()
    {
        var id = await SharedFileAsync();
        var (c, sid) = await OpenAsync(_app, _drive, "u2", id);

        var all = await PeopleAsync(c, sid, "");
        Assert.Equal(["u1", "u2", "u3"], all.Select(p => p.GetProperty("id").GetString()).Order().ToList());
        var cuong = Assert.Single(await PeopleAsync(c, sid, "Cường"));
        Assert.Equal("Lê Văn Cường", cuong.GetProperty("name").GetString());
        Assert.Equal("u3@drive.mock", cuong.GetProperty("email").GetString());
        Assert.Equal("host", cuong.GetProperty("source").GetString());

        // Without the session token: 401.
        var anon = await _app.CreateClient().GetAsync(new Uri($"/api/sessions/{sid}/people?q=a", UriKind.Relative));
        Assert.Equal(HttpStatusCode.Unauthorized, anon.StatusCode);
    }

    [Fact]
    public async Task HostWithoutADirectoryOffersTheCollaborators()
    {
        // A WOPI-like storage (no IHostPeople): only users with a live session on the same file, accents ignored.
        var store = new SessionStore();
        var host = new HostConfig { Name = "wopihost", Protocol = HostProtocol.Wopi };
        EditorSession Session(string user, string name, string file) => new()
        {
            Id = Guid.NewGuid().ToString(),
            Token = "t",
            Host = host,
            Storage = new NoPeopleStorage(),
            FileId = file,
            AccessToken = "x",
            File = new HostFileInfo(file, "a.xlsx", 1, "1", null, new HostPermissions(true, true, true, false, true), new HostUser(user, name, null), null, null, null, false, false),
            ExpiresAt = DateTime.UtcNow.AddHours(1),
        };
        var me = Session("w1", "Nguyễn Văn An", "f");
        store.Add(me);
        store.Add(Session("w2", "Trần Thị Bình", "f"));
        store.Add(Session("w3", "Lê Bình", "other-file"));
        var svc = new CommentMentionService(store, new FakeMailSender(), TimeProvider.System, NullLogger<CommentMentionService>.Instance);

        var hits = await svc.SearchAsync(me, "binh", 10, CancellationToken.None);
        var hit = Assert.Single(hits);
        Assert.Equal(("w2", "collaborator"), (hit.Id, hit.Source));

        // Nothing is sent to a host without notifications; no e-mail without an address.
        var r = await svc.NotifyAsync(me, new CommentMentionRequest("c1", "@Trần Thị Bình xem", [new MentionTarget("w2", "Trần Thị Bình", "spoofed@evil.invalid")], null, "sheet"), CancellationToken.None);
        Assert.Equal(new CommentMentionResult(1, "unsupported", 0), r);
    }

    [Fact]
    public async Task PostedCommentNotifiesTheHostAndSendsEmails()
    {
        var mail = new FakeMailSender();
        using var app = _app.WithWebHostBuilder(b => b.ConfigureServices(s =>
        {
            s.RemoveAll<IMailSender>();
            s.AddSingleton<IMailSender>(mail);
        }));
        var id = await SharedFileAsync();
        var (c, sid) = await OpenAsync(app, _drive, "u1", id);

        var res = await c.PostAsJsonAsync(new Uri($"/api/sessions/{sid}/mentions", UriKind.Relative), new
        {
            commentId = "{C1}",
            text = "@Trần Thị Bình @Lê Văn Cường kiểm tra số liệu",
            app = "doc",
            mentions = new object[]
            {
                // The e-mail sent by the client is ignored: the host's directory decides.
                new { id = "u2", name = "Trần Thị Bình", email = "spoofed@evil.invalid" },
                new { id = "u3", name = "Lê Văn Cường" },
                new { id = "u1", name = "Nguyễn Văn An" }, // the author: not notified
                new { id = "intruder", name = "Kẻ lạ" }, // unknown: not notified
            },
            assignee = new { id = "u2", name = "Trần Thị Bình" },
        });
        Assert.Equal(HttpStatusCode.OK, res.StatusCode);
        var r = await res.Content.ReadFromJsonAsync<JsonElement>();
        Assert.Equal(2, r.GetProperty("recipients").GetInt32());
        Assert.Equal("sent", r.GetProperty("host").GetString());
        Assert.Equal(2, r.GetProperty("emails").GetInt32());

        var notes = (await _drive.Client.GetFromJsonAsync<List<NotificationRecord>>("/_admin/notifications"))!
            .Where(n => n.FileId == id).Select(n => JsonDocument.Parse(n.Body).RootElement).ToList();
        Assert.Equal(2, notes.Count);
        var mention = notes.Single(n => n.GetProperty("type").GetString() == "mention");
        Assert.Equal("u3", Assert.Single(mention.GetProperty("to").EnumerateArray().ToList()).GetProperty("id").GetString());
        Assert.Equal("u1", mention.GetProperty("from").GetProperty("id").GetString());
        Assert.Equal("Kế hoạch.docx", mention.GetProperty("fileName").GetString());
        Assert.Equal("{C1}", mention.GetProperty("commentId").GetString());
        var task = notes.Single(n => n.GetProperty("type").GetString() == "task");
        Assert.Equal("u2@drive.mock", task.GetProperty("to")[0].GetProperty("email").GetString());

        Assert.Equal(["u2@drive.mock", "u3@drive.mock"], mail.Sent.Select(m => m.To).Order().ToList());
        Assert.Contains("giao việc", mail.Sent.Single(m => m.To == "u2@drive.mock").Subject, StringComparison.Ordinal);
        Assert.DoesNotContain(mail.Sent, m => m.To.Contains("evil", StringComparison.Ordinal));
    }

    [Fact]
    public async Task NothingIsMailedWithoutSmtpAndViewersCannotNotify()
    {
        // Default app: no Mail section → the SMTP sender is not configured, nothing is sent.
        Assert.False(_app.Services.GetRequiredService<IMailSender>().IsConfigured);
        var id = await SharedFileAsync();
        var (c, sid) = await OpenAsync(_app, _drive, "u2", id);
        var r = await (await c.PostAsJsonAsync(new Uri($"/api/sessions/{sid}/mentions", UriKind.Relative), new
        {
            commentId = "1",
            text = "@Nguyễn Văn An",
            mentions = new[] { new { id = "u1", name = "Nguyễn Văn An" } },
        })).Content.ReadFromJsonAsync<JsonElement>();
        Assert.Equal(0, r.GetProperty("emails").GetInt32());
        Assert.Equal("sent", r.GetProperty("host").GetString());

        // u3 can only view (no comment permission in the mock): 403.
        await _drive.SetPermsAsync(id, "u3", view: true, edit: false);
        var (v, vsid) = await OpenAsync(_app, _drive, "u3", id);
        var denied = await v.PostAsJsonAsync(new Uri($"/api/sessions/{vsid}/mentions", UriKind.Relative), new { commentId = "1", text = "x", mentions = new[] { new { id = "u1" } } });
        Assert.Equal(HttpStatusCode.Forbidden, denied.StatusCode);
    }

    private sealed class NoPeopleStorage : IHostStorage
    {
        public HostProtocol Protocol => HostProtocol.Wopi;

        public Task<HostFileInfo> GetFileAsync(HostCallContext ctx, string fileId, CancellationToken ct) => throw new NotSupportedException();

        public Task<HostContent> GetContentsAsync(HostCallContext ctx, string fileId, CancellationToken ct) => throw new NotSupportedException();

        public Task<HostPutResult> PutContentsAsync(HostCallContext ctx, string fileId, byte[] content, HostPutOptions options, CancellationToken ct) => throw new NotSupportedException();

        public Task<HostLockResult> LockAsync(HostCallContext ctx, string fileId, string lockId, TimeSpan duration, CancellationToken ct) => throw new NotSupportedException();

        public Task UnlockAsync(HostCallContext ctx, string fileId, string lockId, CancellationToken ct) => throw new NotSupportedException();

        public Task<HostLockResult> RefreshLockAsync(HostCallContext ctx, string fileId, string lockId, TimeSpan duration, CancellationToken ct) => throw new NotSupportedException();

        public Task<IReadOnlyList<HostVersion>> GetVersionsAsync(HostCallContext ctx, string fileId, CancellationToken ct) => throw new NotSupportedException();

        public Task<HostContent> GetVersionContentsAsync(HostCallContext ctx, string fileId, string version, CancellationToken ct) => throw new NotSupportedException();

        public Task<HostPutResult> RestoreVersionAsync(HostCallContext ctx, string fileId, string version, CancellationToken ct) => throw new NotSupportedException();

        public Task<string> RenameAsync(HostCallContext ctx, string fileId, string newName, CancellationToken ct) => throw new NotSupportedException();

        public Task<HostCreatedFile> CreateFileAsync(HostCallContext ctx, string folderId, string name, byte[] content, CancellationToken ct) => throw new NotSupportedException();
    }
}
