using System.Net;
using System.Net.Http.Headers;
using System.Net.Http.Json;
using System.Text;
using System.Text.Json;
using BlueOffice.Api.Presenting;
using BlueOffice.Api.Sessions;
using MockBlueDrive;
using Xunit;

namespace BlueOffice.Tests;

/// <summary>Remote control from a phone and online presentation <c>/api/present</c> (SLIDE-REMOTE-001).</summary>
public sealed class PresentTests : IAsyncLifetime
{
    private readonly TestClock _clock = new();
    private MockDrive _drive = null!;
    private TestApp _app = null!;

    /// <summary>A body that looks like an OPC package (ZIP local file header); the server does not open it.</summary>
    private static readonly byte[] Deck = [(byte)'P', (byte)'K', 3, 4, .. Encoding.ASCII.GetBytes("ppt/presentation.xml")];

    public async Task InitializeAsync()
    {
        _drive = await MockDrive.StartAsync();
        _app = new TestApp(_drive, clock: _clock, settings: new() { ["Present:CommandLimit"] = "5" });
    }

    public async Task DisposeAsync()
    {
        _app.Dispose();
        await _drive.DisposeAsync();
    }

    private sealed record Created(string Id, string Key, bool CanBroadcast);

    private static Uri U(string path) => new(path, UriKind.Relative);

    private async Task<Created> CreateAsync(string user = "alice")
    {
        var r = await _app.ClientFor(user).PostAsync(U("/api/present"), null);
        Assert.Equal(HttpStatusCode.OK, r.StatusCode);
        var j = await r.Content.ReadFromJsonAsync<JsonElement>();
        return new Created(j.GetProperty("id").GetString()!, j.GetProperty("key").GetString()!, j.GetProperty("canBroadcast").GetBoolean());
    }

    private HttpClient Presenter(Created p)
    {
        var c = _app.CreateClient();
        c.DefaultRequestHeaders.Add(PresentEndpoints.KeyHeader, p.Key);
        return c;
    }

    private async Task<string> PairAsync(Created p)
    {
        var r = await Presenter(p).PostAsync(U($"/api/present/{p.Id}/remote"), null);
        Assert.Equal(HttpStatusCode.OK, r.StatusCode);
        var j = await r.Content.ReadFromJsonAsync<JsonElement>();
        var token = j.GetProperty("token").GetString()!;
        Assert.Equal("/remote/" + token, j.GetProperty("path").GetString());
        return token;
    }

    private Task<HttpResponseMessage> ClaimAsync(string token) =>
        _app.CreateClient().PostAsJsonAsync(U("/api/present/remote/claim"), new { token });

    private async Task<string> ClaimKeyAsync(string token)
    {
        var r = await ClaimAsync(token);
        Assert.Equal(HttpStatusCode.OK, r.StatusCode);
        return (await r.Content.ReadFromJsonAsync<JsonElement>()).GetProperty("key").GetString()!;
    }

    private Task<WsClient> PresenterWs(Created p) => _app.ConnectAsync($"/api/present/{p.Id}/ws?key={Uri.EscapeDataString(p.Key)}");

    private Task<WsClient> RemoteWs(string key) => _app.ConnectAsync($"/api/present/remote/ws?key={Uri.EscapeDataString(key)}");

    private Task<WsClient> ViewerWs(string token) => _app.ConnectAsync($"/api/present/watch/ws?token={Uri.EscapeDataString(token)}");

    private async Task<string> BroadcastAsync(Created p)
    {
        var body = new ByteArrayContent(Deck);
        body.Headers.ContentType = new MediaTypeHeaderValue("application/vnd.openxmlformats-officedocument.presentationml.presentation");
        var r = await Presenter(p).PostAsync(U($"/api/present/{p.Id}/broadcast?name=Bai%20giang.pptx"), body);
        Assert.Equal(HttpStatusCode.OK, r.StatusCode);
        return (await r.Content.ReadFromJsonAsync<JsonElement>()).GetProperty("token").GetString()!;
    }

    private Task<HttpResponseMessage> DeckAsync(string? token)
    {
        var c = _app.CreateClient();
        if (token is not null)
        {
            c.DefaultRequestHeaders.Add(PresentEndpoints.TokenHeader, token);
        }

        return c.GetAsync(U("/api/present/watch/deck"));
    }

    /// <summary>A 128-bit token: 22 base64url characters decoding to 16 bytes.</summary>
    private static void Assert128(string token)
    {
        Assert.Matches("^[A-Za-z0-9_-]{22}$", token);
        var b64 = token.Replace('-', '+').Replace('_', '/') + "==";
        Assert.Equal(16, Convert.FromBase64String(b64).Length);
    }

    [Fact]
    public async Task OffWhenDisabled()
    {
        using var off = new TestApp(settings: new() { ["Present:Enabled"] = "false" });
        Assert.Equal(HttpStatusCode.NotFound, (await off.CreateClient().GetAsync(U("/api/present/config"))).StatusCode);
        Assert.Equal(HttpStatusCode.NotFound, (await off.ClientFor("alice").PostAsync(U("/api/present"), null)).StatusCode);
        Assert.Equal(HttpStatusCode.OK, (await _app.CreateClient().GetAsync(U("/api/present/config"))).StatusCode);
    }

    [Fact]
    public async Task CreatingNeedsASignedInUserOrAnEditorSession()
    {
        Assert.Equal(HttpStatusCode.Unauthorized, (await _app.CreateClient().PostAsync(U("/api/present"), null)).StatusCode);
        Assert.Equal(HttpStatusCode.Unauthorized, (await _app.CreateClient().PostAsync(U("/api/present?session=nope"), null)).StatusCode);
        var p = await CreateAsync();
        Assert128(p.Key);
        Assert128(p.Id);
        Assert.True(p.CanBroadcast);

        // A hosted file: the editor session's secret.
        var fileId = await _drive.CreateFileAsync("Bài giảng.pptx", "deck");
        var token = await _drive.TokenAsync("u1", fileId);
        var s = await (await _app.CreateClient().PostAsJsonAsync(U("/api/sessions"), new { host = "bluedrive", fileId, token })).Content.ReadFromJsonAsync<JsonElement>();
        var sid = s.GetProperty("sessionId").GetString()!;
        var wrong = _app.CreateClient();
        wrong.DefaultRequestHeaders.Add(SessionEndpoints.TokenHeader, "not-the-secret");
        Assert.Equal(HttpStatusCode.Unauthorized, (await wrong.PostAsync(U($"/api/present?session={sid}"), null)).StatusCode);
        var c = _app.CreateClient();
        c.DefaultRequestHeaders.Add(SessionEndpoints.TokenHeader, s.GetProperty("sessionToken").GetString());
        var r = await c.PostAsync(U($"/api/present?session={sid}"), null);
        Assert.Equal(HttpStatusCode.OK, r.StatusCode);
        Assert.True((await r.Content.ReadFromJsonAsync<JsonElement>()).GetProperty("canBroadcast").GetBoolean());

        // The presenter key is needed for everything else.
        var other = _app.CreateClient();
        other.DefaultRequestHeaders.Add(PresentEndpoints.KeyHeader, "wrong");
        Assert.Equal(HttpStatusCode.Unauthorized, (await other.PostAsync(U($"/api/present/{p.Id}/remote"), null)).StatusCode);
        Assert.Equal(HttpStatusCode.Unauthorized, (await _app.CreateClient().PostAsync(U($"/api/present/{p.Id}/broadcast"), new ByteArrayContent(Deck))).StatusCode);
        await Assert.ThrowsAnyAsync<Exception>(() => _app.ConnectAsync($"/api/present/{p.Id}/ws?key=wrong"));
    }

    [Fact]
    public async Task OnlyAFewPresentationsPerUser()
    {
        for (var i = 0; i < 5; i++)
        {
            await CreateAsync("bob");
        }

        var r = await _app.ClientFor("bob").PostAsync(U("/api/present"), null);
        Assert.Equal(HttpStatusCode.TooManyRequests, r.StatusCode);
        Assert.Equal("tooMany", (await r.Content.ReadFromJsonAsync<JsonElement>()).GetProperty("code").GetString());

        // Ending one frees a place.
        var p = await CreateAsync("carol");
        Assert.Equal(HttpStatusCode.NoContent, (await Presenter(p).DeleteAsync(U($"/api/present/{p.Id}"))).StatusCode);
        Assert.Equal(HttpStatusCode.Unauthorized, (await Presenter(p).PostAsync(U($"/api/present/{p.Id}/remote"), null)).StatusCode);
    }

    [Fact]
    public async Task ThePhoneControlsItsPresenterOnly()
    {
        var a = await CreateAsync("alice");
        var b = await CreateAsync("bob");
        await using var wa = await PresenterWs(a);
        await using var wb = await PresenterWs(b);
        var first = await wa.WaitEventAsync("peers");
        Assert.False(first.GetProperty("remote").GetBoolean());

        var pairing = await PairAsync(a);
        Assert128(pairing);
        var key = await ClaimKeyAsync(pairing);
        Assert128(key);
        Assert.NotEqual(pairing, key);

        // The code works once.
        Assert.Equal(HttpStatusCode.NotFound, (await ClaimAsync(pairing)).StatusCode);

        // The pairing code itself does not connect; the phone's key does.
        await Assert.ThrowsAnyAsync<Exception>(() => RemoteWs(pairing));
        await using var phone = await RemoteWs(key);
        Assert.True((await phone.WaitEventAsync("peers")).GetProperty("presenter").GetBoolean());
        await wa.WaitAsync(m => m.EventType == "peers" && m.Json.GetProperty("remote").GetBoolean(), "phone connected");

        // State for the phone (current / next slide, notes) goes to the phone.
        await wa.SendJsonAsync(new { type = "remote", slide = 2, total = 5, notes = "Ghi chú" });
        var state = await phone.WaitEventAsync("remote");
        Assert.Equal(2, state.GetProperty("slide").GetInt32());
        Assert.Equal("Ghi chú", state.GetProperty("notes").GetString());

        // Commands reach that presenter, re-serialised; the other presenter gets nothing.
        await phone.SendJsonAsync(new { type = "command", action = "next", extra = "<script>" });
        var cmd = await wa.WaitEventAsync("command");
        Assert.Equal("next", cmd.GetProperty("action").GetString());
        Assert.False(cmd.TryGetProperty("extra", out _));
        await phone.SendJsonAsync(new { type = "command", action = "prev" });
        Assert.Equal("prev", (await wa.WaitEventAsync("command")).GetProperty("action").GetString());

        // Unknown actions are refused.
        await phone.SendJsonAsync(new { type = "command", action = "deleteSlide" });
        Assert.Equal("invalid", (await phone.WaitEventAsync("error")).GetProperty("code").GetString());
        await Task.Delay(100);
        Assert.DoesNotContain(wb.Messages, m => m.EventType == "command");
        Assert.DoesNotContain(wa.Messages, m => m.EventType == "command" && m.Json.GetProperty("action").GetString() == "deleteSlide");

        // A phone reconnecting gets the last state at once.
        await phone.DisposeAsync();
        await using var again = await RemoteWs(key);
        Assert.Equal(2, (await again.WaitEventAsync("remote")).GetProperty("slide").GetInt32());

        // The presenter away: the phone is told.
        await wa.DisposeAsync();
        await again.WaitAsync(m => m.EventType == "peers" && !m.Json.GetProperty("presenter").GetBoolean(), "presenter left");
        await again.SendJsonAsync(new { type = "command", action = "next" });
        Assert.Equal("presenterOffline", (await again.WaitEventAsync("error")).GetProperty("code").GetString());
    }

    [Fact]
    public async Task CommandsAreRateLimited()
    {
        var a = await CreateAsync();
        await using var wa = await PresenterWs(a);
        await using var phone = await RemoteWs(await ClaimKeyAsync(await PairAsync(a)));
        for (var i = 0; i < 6; i++)
        {
            await phone.SendJsonAsync(new { type = "command", action = "next" });
        }

        Assert.Equal("rateLimited", (await phone.WaitEventAsync("error")).GetProperty("code").GetString());
        await Task.Delay(100);
        Assert.Equal(5, wa.Messages.Count(m => m.EventType == "command"));

        // A new window.
        _clock.Advance(TimeSpan.FromSeconds(11));
        await phone.SendJsonAsync(new { type = "command", action = "next" });
        await wa.WaitAsync(m => m.EventType == "command" && wa.Messages.Count(x => x.EventType == "command") == 6, "command after the window");
    }

    [Fact]
    public async Task PairingCodesExpire()
    {
        var a = await CreateAsync();
        var pairing = await PairAsync(a);
        _clock.Advance(TimeSpan.FromMinutes(5) + TimeSpan.FromSeconds(1));
        var r = await ClaimAsync(pairing);
        Assert.Equal(HttpStatusCode.NotFound, r.StatusCode);
        Assert.Equal("invalidToken", (await r.Content.ReadFromJsonAsync<JsonElement>()).GetProperty("code").GetString());

        // Within the time it works.
        var fresh = await PairAsync(a);
        _clock.Advance(TimeSpan.FromMinutes(4));
        Assert.Equal(HttpStatusCode.OK, (await ClaimAsync(fresh)).StatusCode);
    }

    [Fact]
    public async Task ThePresenterRevokesThePhone()
    {
        var a = await CreateAsync();
        await using var wa = await PresenterWs(a);
        await wa.WaitEventAsync("peers");
        var key = await ClaimKeyAsync(await PairAsync(a));
        var phone = await RemoteWs(key);
        await phone.WaitEventAsync("peers");
        await wa.WaitAsync(m => m.EventType == "peers" && m.Json.GetProperty("remote").GetBoolean(), "phone connected");
        Assert.Equal(HttpStatusCode.NoContent, (await Presenter(a).DeleteAsync(U($"/api/present/{a.Id}/remote"))).StatusCode);
        await phone.WaitEventAsync("revoked");
        await phone.WaitClosedAsync();
        Assert.Equal(4401, (int?)phone.CloseStatus);
        await phone.DisposeAsync();
        await wa.WaitAsync(m => m.EventType == "peers" && !m.Json.GetProperty("paired").GetBoolean(), "phone gone");
        await Assert.ThrowsAnyAsync<Exception>(() => RemoteWs(key));

        // A new pairing code replaces the old one (and the phone paired with it).
        var p1 = await PairAsync(a);
        var p2 = await PairAsync(a);
        Assert.Equal(HttpStatusCode.NotFound, (await ClaimAsync(p1)).StatusCode);
        Assert.Equal(HttpStatusCode.OK, (await ClaimAsync(p2)).StatusCode);
    }

    [Fact]
    public async Task ViewersFollowAndCannotSendAnything()
    {
        var a = await CreateAsync();
        await using var wa = await PresenterWs(a);
        await wa.WaitEventAsync("peers");

        // Before the online presentation starts, show states go nowhere.
        await wa.SendJsonAsync(new { type = "show", slide = 0, click = 0 });
        var token = await BroadcastAsync(a);
        Assert128(token);

        Assert.Equal(HttpStatusCode.NotFound, (await DeckAsync(null)).StatusCode);
        Assert.Equal(HttpStatusCode.NotFound, (await DeckAsync("wrong-token")).StatusCode);
        var deck = await DeckAsync(token);
        Assert.Equal(HttpStatusCode.OK, deck.StatusCode);
        Assert.Equal(Deck, await deck.Content.ReadAsByteArrayAsync());
        Assert.Equal("no-store", deck.Headers.CacheControl?.ToString());

        await wa.SendJsonAsync(new { type = "show", slide = 1, click = 2, enter = "click" });
        await Task.Delay(100);
        await using var viewer = await ViewerWs(token);

        // The last state at once, then each new one.
        var last = await viewer.WaitEventAsync("show");
        Assert.Equal(1, last.GetProperty("slide").GetInt32());
        Assert.Equal(2, last.GetProperty("click").GetInt32());
        await wa.WaitAsync(m => m.EventType == "peers" && m.Json.GetProperty("viewers").GetInt32() == 1, "one viewer");
        await wa.SendJsonAsync(new { type = "show", slide = 3, click = 0, enter = "transition" });
        Assert.Equal(3, (await viewer.WaitEventAsync("show")).GetProperty("slide").GetInt32());

        // Viewers cannot move the show (nor talk to the presenter or the phone).
        await viewer.SendJsonAsync(new { type = "command", action = "next" });
        await viewer.SendJsonAsync(new { type = "show", slide = 9 });
        await Task.Delay(150);
        Assert.DoesNotContain(wa.Messages, m => m.EventType is "command" or "show");

        // The phone's state is not sent to viewers (notes stay with the presenter).
        await wa.SendJsonAsync(new { type = "remote", notes = "bí mật" });
        await Task.Delay(100);
        Assert.DoesNotContain(viewer.Messages, m => m.Text.Contains("bí mật", StringComparison.Ordinal));

        // Refreshing the deck keeps the link and tells viewers to reload.
        var body = new ByteArrayContent(Deck);
        var refreshed = await (await Presenter(a).PostAsync(U($"/api/present/{a.Id}/broadcast"), body)).Content.ReadFromJsonAsync<JsonElement>();
        Assert.True(refreshed.GetProperty("refreshed").GetBoolean());
        await viewer.WaitEventAsync("reload");

        // The presenter ends it: viewers are told, the link stops working.
        Assert.Equal(HttpStatusCode.NoContent, (await Presenter(a).DeleteAsync(U($"/api/present/{a.Id}/broadcast"))).StatusCode);
        await viewer.WaitEventAsync("ended");
        await viewer.WaitClosedAsync();
        Assert.Equal(4410, (int?)viewer.CloseStatus);
        Assert.Equal(HttpStatusCode.NotFound, (await DeckAsync(token)).StatusCode);
        await Assert.ThrowsAnyAsync<Exception>(() => ViewerWs(token));
    }

    [Fact]
    public async Task OnlyPackagesWithinTheLimitAndOnlyWhenDownloadIsAllowed()
    {
        var a = await CreateAsync();
        var r = await Presenter(a).PostAsync(U($"/api/present/{a.Id}/broadcast"), new ByteArrayContent(Encoding.ASCII.GetBytes("<html>")));
        Assert.Equal(HttpStatusCode.BadRequest, r.StatusCode);

        using var small = new TestApp(settings: new() { ["Present:MaxDeckBytes"] = "16" });
        var c = small.ClientFor("alice");
        var j = await (await c.PostAsync(U("/api/present"), null)).Content.ReadFromJsonAsync<JsonElement>();
        var pc = small.CreateClient();
        pc.DefaultRequestHeaders.Add(PresentEndpoints.KeyHeader, j.GetProperty("key").GetString());
        r = await pc.PostAsync(U($"/api/present/{j.GetProperty("id").GetString()}/broadcast"), new ByteArrayContent([.. Deck, .. new byte[32]]));
        Assert.Equal(HttpStatusCode.RequestEntityTooLarge, r.StatusCode);

        // A hosted file the user may not download cannot be handed to viewers (remote control still works).
        var fileId = await _drive.CreateFileAsync("Nội bộ.pptx", "deck");
        var perms = await _drive.Client.PutAsJsonAsync($"/_admin/files/{fileId}/permissions/u2", new Perms(true, false, false, false, false));
        perms.EnsureSuccessStatusCode();
        var token = await _drive.TokenAsync("u2", fileId);
        var s = await (await _app.CreateClient().PostAsJsonAsync(U("/api/sessions"), new { host = "bluedrive", fileId, token })).Content.ReadFromJsonAsync<JsonElement>();
        var sc = _app.CreateClient();
        sc.DefaultRequestHeaders.Add(SessionEndpoints.TokenHeader, s.GetProperty("sessionToken").GetString());
        var created = await (await sc.PostAsync(U($"/api/present?session={s.GetProperty("sessionId").GetString()}"), null)).Content.ReadFromJsonAsync<JsonElement>();
        Assert.False(created.GetProperty("canBroadcast").GetBoolean());
        var p = new Created(created.GetProperty("id").GetString()!, created.GetProperty("key").GetString()!, false);
        r = await Presenter(p).PostAsync(U($"/api/present/{p.Id}/broadcast"), new ByteArrayContent(Deck));
        Assert.Equal(HttpStatusCode.Forbidden, r.StatusCode);
        Assert.Equal(HttpStatusCode.OK, (await Presenter(p).PostAsync(U($"/api/present/{p.Id}/remote"), null)).StatusCode);
    }

    [Fact]
    public async Task EndingThePresentationEndsEveryToken()
    {
        var a = await CreateAsync();
        await using var wa = await PresenterWs(a);
        var key = await ClaimKeyAsync(await PairAsync(a));
        await using var phone = await RemoteWs(key);
        var token = await BroadcastAsync(a);
        await using var viewer = await ViewerWs(token);
        await phone.WaitEventAsync("peers");

        Assert.Equal(HttpStatusCode.NoContent, (await Presenter(a).DeleteAsync(U($"/api/present/{a.Id}"))).StatusCode);
        foreach (var ws in new[] { phone, viewer, wa })
        {
            await ws.WaitEventAsync("ended");
            await ws.WaitClosedAsync();
        }

        Assert.Equal(HttpStatusCode.NotFound, (await DeckAsync(token)).StatusCode);
        await Assert.ThrowsAnyAsync<Exception>(() => RemoteWs(key));
        await Assert.ThrowsAnyAsync<Exception>(() => PresenterWs(a));
    }

    [Fact]
    public async Task PresentationsExpire()
    {
        var a = await CreateAsync();
        var token = await BroadcastAsync(a);
        var key = await ClaimKeyAsync(await PairAsync(a));
        Assert.Equal(HttpStatusCode.OK, (await DeckAsync(token)).StatusCode);

        // No presenter connected for the idle timeout (30 minutes).
        _clock.Advance(TimeSpan.FromMinutes(31));
        Assert.Equal(HttpStatusCode.NotFound, (await DeckAsync(token)).StatusCode);
        await Assert.ThrowsAnyAsync<Exception>(() => RemoteWs(key));

        // A connected presenter keeps it alive until the hard limit (12 hours).
        var b = await CreateAsync();
        await using var wb = await PresenterWs(b);
        // The server announces `peers` once it has registered the presenter's socket; before
        // that the presentation still counts as idle (seen on a slow CI runner).
        await wb.WaitEventAsync("peers");
        var tb = await BroadcastAsync(b);
        _clock.Advance(TimeSpan.FromHours(2));
        Assert.Equal(HttpStatusCode.OK, (await DeckAsync(tb)).StatusCode);
        _clock.Advance(TimeSpan.FromHours(10));
        Assert.Equal(HttpStatusCode.NotFound, (await DeckAsync(tb)).StatusCode);
        await wb.WaitEventAsync("ended");
    }

    [Fact]
    public async Task PairingAttemptsAreRateLimited()
    {
        using var app = new TestApp(settings: new() { ["Present:RequestLimit"] = "3" });
        var c = app.CreateClient();
        for (var i = 0; i < 3; i++)
        {
            Assert.Equal(HttpStatusCode.NotFound, (await c.PostAsJsonAsync(U("/api/present/remote/claim"), new { token = PresentHub.NewToken() })).StatusCode);
        }

        Assert.Equal(HttpStatusCode.TooManyRequests, (await c.PostAsJsonAsync(U("/api/present/remote/claim"), new { token = PresentHub.NewToken() })).StatusCode);
    }
}
