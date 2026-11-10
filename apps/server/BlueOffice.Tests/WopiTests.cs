using System.Collections.Concurrent;
using System.Globalization;
using System.Net;
using System.Net.Http.Json;
using System.Text;
using System.Text.Json;
using BlueOffice.Api.Sessions;
using BlueOffice.Infrastructure.Hosts;
using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Http;
using Microsoft.AspNetCore.TestHost;
using Xunit;

namespace BlueOffice.Tests;

/// <summary>Minimal WOPI host ([MS-WOPI] §3.3.5) used to test BlueOffice as a WOPI client.</summary>
public sealed class FakeWopiHost : IAsyncDisposable
{
    /// <summary>Access token accepted by the host.</summary>
    public const string Token = "wopi-access-token";

    private int _proofChecks;
    private int _proofFailures;

    private FakeWopiHost(WebApplication app)
    {
        App = app;
        Server = app.GetTestServer();
    }

    /// <summary>App.</summary>
    public WebApplication App { get; }

    /// <summary>Server.</summary>
    public TestServer Server { get; }

    /// <summary>Content by file id.</summary>
    public ConcurrentDictionary<string, byte[]> Files { get; } = new() { ["doc1"] = "wopi v1"u8.ToArray() };

    /// <summary>Version by file id.</summary>
    public ConcurrentDictionary<string, int> Versions { get; } = new() { ["doc1"] = 1 };

    /// <summary>Lock by file id.</summary>
    public ConcurrentDictionary<string, string> Locks { get; } = new();

    /// <summary>Validates the proof headers of every request when set ([MS-WOPI] proof keys).</summary>
    public WopiProofValidator? Proofs { get; set; }

    /// <summary>Number of requests whose proof was checked.</summary>
    public int ProofChecks => _proofChecks;

    /// <summary>Number of requests with an invalid proof.</summary>
    public int ProofFailures => _proofFailures;

    /// <summary>CheckFileInfo sends <c>Version</c> as a JSON number (seen in some hosts).</summary>
    public bool NumericVersion { get; set; }

    /// <summary>Overrides received ("LOCK", "PUT", …) with the X-WOPI-Lock value.</summary>
    public ConcurrentQueue<(string Op, string? Lock)> Calls { get; } = new();

    /// <summary>Starts the host.</summary>
    public static async Task<FakeWopiHost> StartAsync()
    {
        var builder = WebApplication.CreateBuilder();
        builder.WebHost.UseTestServer();
        var app = builder.Build();
        var host = new FakeWopiHost(app);
        host.Map(app);
        await app.StartAsync();
        return host;
    }

    private void Map(WebApplication app)
    {
        app.Use(async (ctx, next) =>
        {
            // [MS-WOPI] §3.3.1: access_token query parameter.
            if (ctx.Request.Query["access_token"] != Token)
            {
                ctx.Response.StatusCode = 401;
                return;
            }

            if (Proofs is { } proofs)
            {
                // The host checks that the request comes from the WOPI client it trusts (discovery proof keys).
                Interlocked.Increment(ref _proofChecks);
                var r = ctx.Request;
                var url = $"{r.Scheme}://{r.Host}{r.PathBase}{r.Path}{r.QueryString}";
                if (!proofs.IsValid(Token, url, r.Headers[WopiProof.ProofHeader], r.Headers[WopiProof.ProofOldHeader], r.Headers[WopiProof.TimeStampHeader], DateTimeOffset.UtcNow))
                {
                    Interlocked.Increment(ref _proofFailures);
                    ctx.Response.StatusCode = 500;
                    return;
                }
            }

            await next();
        });

        // CheckFileInfo §3.3.5.1.1
        app.MapGet("/wopi/files/{id}", (string id) => Files.TryGetValue(id, out var c)
            ? Results.Json(new
            {
                BaseFileName = id + ".docx",
                OwnerId = "owner",
                Size = c.Length,
                Version = NumericVersion ? (object)Versions[id] : Versions[id].ToString(CultureInfo.InvariantCulture),
                UserId = "wopi-user",
                UserFriendlyName = "Phạm Minh",
                UserCanWrite = true,
                SupportsLocks = true,
                SupportsUpdate = true,
                SupportsRename = true,
                UserCanRename = true,
            })
            : Results.NotFound());

        // GetFile §3.3.5.3.1
        app.MapGet("/wopi/files/{id}/contents", (string id, HttpResponse res) =>
        {
            res.Headers["X-WOPI-ItemVersion"] = Versions[id].ToString(CultureInfo.InvariantCulture);
            return Results.Bytes(Files[id]);
        });

        // PutFile §3.3.5.3.2
        app.MapPost("/wopi/files/{id}/contents", async (string id, HttpRequest req, HttpResponse res) =>
        {
            var l = req.Headers["X-WOPI-Lock"].ToString();
            Calls.Enqueue(("PUT", l));
            if (Locks.TryGetValue(id, out var current) && current != l)
            {
                res.Headers["X-WOPI-Lock"] = current;
                return Results.StatusCode(409);
            }

            using var ms = new MemoryStream();
            await req.Body.CopyToAsync(ms);
            Files[id] = ms.ToArray();
            var v = Versions.AddOrUpdate(id, 1, (_, x) => x + 1);
            res.Headers["X-WOPI-ItemVersion"] = v.ToString(CultureInfo.InvariantCulture);
            return Results.Ok();
        });

        // Lock / Unlock / RefreshLock / PutRelativeFile / RenameFile (X-WOPI-Override)
        app.MapPost("/wopi/files/{id}", async (string id, HttpRequest req, HttpResponse res) =>
        {
            var op = req.Headers["X-WOPI-Override"].ToString();
            var l = req.Headers["X-WOPI-Lock"].ToString();
            Calls.Enqueue((op, l));
            var held = Locks.TryGetValue(id, out var current);
            switch (op)
            {
                case "LOCK" or "REFRESH_LOCK" or "UNLOCK" when held && current != l:
                    res.Headers["X-WOPI-Lock"] = current;
                    return Results.StatusCode(409);
                case "LOCK" or "REFRESH_LOCK":
                    Locks[id] = l;
                    return Results.Ok();
                case "UNLOCK":
                    Locks.TryRemove(id, out _);
                    return Results.Ok();
                case "PUT_RELATIVE":
                    {
                        var name = req.Headers["X-WOPI-SuggestedTarget"].ToString();
                        using var ms = new MemoryStream();
                        await req.Body.CopyToAsync(ms);
                        var newId = "rel" + Files.Count.ToString(CultureInfo.InvariantCulture);
                        Files[newId] = ms.ToArray();
                        Versions[newId] = 1;
                        return Results.Json(new { Name = name, Url = $"http://wopi/wopi/files/{newId}?access_token=x" });
                    }

                default:
                    return Results.StatusCode(501);
            }
        });
    }

    /// <inheritdoc />
    public async ValueTask DisposeAsync()
    {
        await App.StopAsync();
        await App.DisposeAsync();
    }
}

public sealed class WopiTests : IAsyncLifetime
{
    private FakeWopiHost _wopi = null!;
    private TestApp _app = null!;

    public async Task InitializeAsync()
    {
        _wopi = await FakeWopiHost.StartAsync();
        _app = new TestApp(wopi: _wopi.Server);

        // Like a real WOPI host: trust the proof keys published in the BlueOffice discovery.
        var discovery = System.Xml.Linq.XDocument.Parse(await _app.CreateClient().GetStringAsync(new Uri("/hosting/discovery", UriKind.Relative)));
        _wopi.Proofs = WopiProofValidator.FromDiscovery(discovery);
    }

    public async Task DisposeAsync()
    {
        _app.Dispose();
        await _wopi.DisposeAsync();
    }

    private async Task<JsonElement> OpenAsync()
    {
        // WOPI hosts POST the access token as a form field to the action URL ([MS-WOPI] §3.1.5.1.1.2).
        using var form = new FormUrlEncodedContent([new("access_token", FakeWopiHost.Token)]);
        var res = await _app.CreateClient().PostAsync(new Uri("/edit?WOPISrc=" + Uri.EscapeDataString("http://wopi/wopi/files/doc1"), UriKind.Relative), form);
        Assert.Equal(HttpStatusCode.OK, res.StatusCode);
        return await res.Content.ReadFromJsonAsync<JsonElement>();
    }

    [Fact]
    public async Task OpensEditsSavesUnderLockAndUnlocks()
    {
        var s = await OpenAsync();
        Assert.Equal("wopi", s.GetProperty("protocol").GetString());
        Assert.Equal("edit", s.GetProperty("mode").GetString());
        Assert.Equal("doc1.docx", s.GetProperty("file").GetProperty("name").GetString());
        Assert.Equal("Phạm Minh", s.GetProperty("user").GetProperty("name").GetString());

        var sid = s.GetProperty("sessionId").GetString();
        var c = _app.CreateClient();
        c.DefaultRequestHeaders.Add(SessionEndpoints.TokenHeader, s.GetProperty("sessionToken").GetString());
        Assert.Equal("wopi v1", Encoding.UTF8.GetString(await c.GetByteArrayAsync(new Uri($"/api/sessions/{sid}/contents", UriKind.Relative))));

        await using (var ws = await _app.ConnectAsync($"{s.GetProperty("collab").GetProperty("url").GetString()}&token={s.GetProperty("sessionToken").GetString()}"))
        {
            await ws.WaitEventAsync("welcome");
            await CollabTests.Eventually(() => _wopi.Locks.ContainsKey("doc1"));
            await ws.SendAsync([0, 2, 1, 4]);
            var r = await c.PostAsync(new Uri($"/api/sessions/{sid}/checkpoint?save=true", UriKind.Relative), new ByteArrayContent("wopi v2"u8.ToArray()));
            var body = await r.Content.ReadFromJsonAsync<JsonElement>();
            Assert.Equal("saved", body.GetProperty("outcome").GetString());
            Assert.Equal("2", body.GetProperty("version").GetString());
        }

        Assert.Equal("wopi v2", Encoding.UTF8.GetString(_wopi.Files["doc1"]));
        await CollabTests.Eventually(() => !_wopi.Locks.ContainsKey("doc1"));
        var calls = _wopi.Calls.ToList();
        var lockId = calls.First(x => x.Op == "LOCK").Lock;
        Assert.False(string.IsNullOrEmpty(lockId));
        Assert.Equal(lockId, calls.First(x => x.Op == "PUT").Lock);
        Assert.Contains(calls, x => x.Op == "UNLOCK" && x.Lock == lockId);

        // WOPI has no version history.
        Assert.Equal(HttpStatusCode.NotImplemented, (await c.GetAsync(new Uri($"/api/sessions/{sid}/versions", UriKind.Relative))).StatusCode);
        Assert.Equal(HttpStatusCode.NotImplemented, (await c.GetAsync(new Uri($"/api/sessions/{sid}/versions/1/contents", UriKind.Relative))).StatusCode);
    }

    [Fact]
    public async Task EveryRequestCarriesAValidProofAndTheSessionFollowsTheFileVersion()
    {
        _wopi.NumericVersion = true;
        _wopi.Versions["doc1"] = 41;
        var s = await OpenAsync();
        Assert.Equal("41", s.GetProperty("file").GetProperty("version").GetString());
        var sid = s.GetProperty("sessionId").GetString();
        var c = _app.CreateClient();
        c.DefaultRequestHeaders.Add(SessionEndpoints.TokenHeader, s.GetProperty("sessionToken").GetString());

        // No collaboration room: the checkpoint is written with PutFile and returns X-WOPI-ItemVersion.
        var r = await (await c.PostAsync(new Uri($"/api/sessions/{sid}/checkpoint?save=true", UriKind.Relative), new ByteArrayContent("wopi v42"u8.ToArray())))
            .Content.ReadFromJsonAsync<JsonElement>();
        Assert.Equal("42", r.GetProperty("version").GetString());
        var info = await c.GetFromJsonAsync<JsonElement>(new Uri($"/api/sessions/{sid}", UriKind.Relative));
        Assert.Equal("42", info.GetProperty("file").GetProperty("version").GetString());

        // Offline sync against the current version is a plain save (no conflict copy).
        var sync = await (await c.PostAsync(new Uri($"/api/sessions/{sid}/sync?baseVersion=42", UriKind.Relative), new ByteArrayContent("offline"u8.ToArray())))
            .Content.ReadFromJsonAsync<JsonElement>();
        Assert.Equal("saved", sync.GetProperty("outcome").GetString());

        Assert.True(_wopi.ProofChecks >= 3, $"{_wopi.ProofChecks} checks");
        Assert.Equal(0, _wopi.ProofFailures);
    }

    [Fact]
    public async Task HostRejectsRequestsWithoutAValidProof()
    {
        // A host that trusts another client's keys refuses BlueOffice (the session cannot open).
        using var other = System.Security.Cryptography.RSA.Create(2048);
        _wopi.Proofs = new WopiProofValidator(other, null);
        using var form = new FormUrlEncodedContent([new("access_token", FakeWopiHost.Token)]);
        var res = await _app.CreateClient().PostAsync(new Uri("/edit?WOPISrc=" + Uri.EscapeDataString("http://wopi/wopi/files/doc1"), UriKind.Relative), form);
        Assert.False(res.IsSuccessStatusCode);
        Assert.True(_wopi.ProofFailures > 0);
    }

    [Fact]
    public async Task ConflictCopyUsesPutRelativeFile()
    {
        var s = await OpenAsync();
        _wopi.Files["doc1"] = "changed elsewhere"u8.ToArray();
        _wopi.Versions["doc1"] = 5;
        var c = _app.CreateClient();
        c.DefaultRequestHeaders.Add(SessionEndpoints.TokenHeader, s.GetProperty("sessionToken").GetString());
        var r = await (await c.PostAsync(new Uri($"/api/sessions/{s.GetProperty("sessionId").GetString()}/sync?baseVersion=1", UriKind.Relative), new ByteArrayContent("offline"u8.ToArray())))
            .Content.ReadFromJsonAsync<JsonElement>();
        Assert.Equal("conflict-copy", r.GetProperty("outcome").GetString());
        var id = r.GetProperty("conflictCopy").GetProperty("id").GetString()!;
        Assert.Equal("offline", Encoding.UTF8.GetString(_wopi.Files[id]));
        Assert.Equal("changed elsewhere", Encoding.UTF8.GetString(_wopi.Files["doc1"]));

        // The suggested target is UTF-7 encoded ([MS-WOPI] §3.3.5.4.1).
        Assert.Contains(_wopi.Calls, x => x.Op == "PUT_RELATIVE");
    }

    [Fact]
    public async Task RejectsForeignWopiSrcAndBadToken()
    {
        var anon = _app.CreateClient();
        Assert.Equal(HttpStatusCode.Forbidden, (await anon.GetAsync(new Uri("/edit?WOPISrc=" + Uri.EscapeDataString("http://evil/wopi/files/doc1") + "&access_token=x", UriKind.Relative))).StatusCode);
        Assert.Equal(HttpStatusCode.Forbidden, (await anon.GetAsync(new Uri("/edit?WOPISrc=" + Uri.EscapeDataString("http://wopi/wopi/files/doc1") + "&access_token=wrong", UriKind.Relative))).StatusCode);
    }

    [Fact]
    public async Task DiscoveryListsTheThreeApps()
    {
        var xml = await _app.CreateClient().GetStringAsync(new Uri("/hosting/discovery", UriKind.Relative));
        var doc = System.Xml.Linq.XDocument.Parse(xml);
        var apps = doc.Descendants("app").Select(a => (string?)a.Attribute("name")).ToList();
        Assert.Equal<string?>(["BlueDoc", "BlueSheet", "BlueSlide"], apps);
        Assert.Contains(doc.Descendants("action"), a => (string?)a.Attribute("ext") == "xlsx" && (string?)a.Attribute("name") == "edit");
        Assert.All(doc.Descendants("action"), a => Assert.StartsWith("http://localhost/edit?", (string?)a.Attribute("urlsrc"), StringComparison.Ordinal));

        // proof-key: CSP PUBLICKEYBLOB (value) + modulus/exponent; no rotation configured, so old = current.
        var key = Assert.Single(doc.Descendants("proof-key"));
        var blob = Convert.FromBase64String((string)key.Attribute("value")!);
        var modulus = Convert.FromBase64String((string)key.Attribute("modulus")!);
        Assert.Equal(256, modulus.Length);
        Assert.Equal(20 + 256, blob.Length);
        Assert.Equal(new byte[] { 0x06, 0x02, 0, 0, 0x00, 0xA4, 0, 0, (byte)'R', (byte)'S', (byte)'A', (byte)'1' }, blob.Take(12).ToArray());
        Assert.Equal(modulus[^1], blob[20]);
        Assert.Equal("AQAB", (string?)key.Attribute("exponent"));
        Assert.Equal((string?)key.Attribute("value"), (string?)key.Attribute("oldvalue"));
    }

    [Fact]
    public void Utf7EncodesNonAscii() =>
        Assert.Equal("B+AOE-o c+AOE-o", Utf7.Encode("Báo cáo"));
}
