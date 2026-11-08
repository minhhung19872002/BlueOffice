using System.Net;
using System.Net.Http.Json;
using System.Net.WebSockets;
using System.Text.Json;
using BlueOffice.Api.Collab;
using Microsoft.Extensions.DependencyInjection;
using Xunit;

namespace BlueOffice.Tests;

public sealed class YProtocolTests
{
    [Theory]
    [InlineData(0UL)]
    [InlineData(127UL)]
    [InlineData(128UL)]
    [InlineData(300UL)]
    [InlineData(ulong.MaxValue)]
    public void VarUintRoundTrips(ulong value)
    {
        var w = new System.Buffers.ArrayBufferWriter<byte>();
        YProtocol.WriteVarUint(w, value);
        var pos = 0;
        Assert.Equal(value, YProtocol.ReadVarUint(w.WrittenSpan, ref pos));
        Assert.Equal(w.WrittenCount, pos);
    }

    [Fact]
    public void EncodesSyncAndAwarenessLikeYProtocols()
    {
        // y-protocols: writeVarUint(messageSync) writeVarUint(messageYjsUpdate) writeVarUint8Array(update)
        Assert.Equal(new byte[] { 0, 2, 3, 1, 2, 3 }, YProtocol.EncodeSync(YProtocol.SyncUpdate, [1, 2, 3]));
        Assert.Equal(new byte[] { 0, 0, 1, 0 }, YProtocol.EncodeSync(YProtocol.SyncStep1, YProtocol.EmptyStateVector));
        var msg = YProtocol.EncodeAwareness([new AwarenessEntry(300, 2, "{\"a\":1}")]);
        var pos = 0;
        Assert.Equal(YProtocol.MessageAwareness, YProtocol.ReadVarUint(msg, ref pos));
        var entries = YProtocol.DecodeAwareness(YProtocol.ReadVarUint8Array(msg, ref pos));
        Assert.Equal(new AwarenessEntry(300, 2, "{\"a\":1}"), Assert.Single(entries));
    }

    [Fact]
    public void FramesRoundTripAndTruncationIsDetected()
    {
        var frames = YProtocol.EncodeFrames([[1], [], [2, 3]]);
        Assert.Equal(3, YProtocol.DecodeFrames(frames).Count);
        Assert.Throws<FormatException>(() => YProtocol.DecodeFrames(new byte[] { 5, 1 }));
    }
}

public sealed class CollabTests : IDisposable
{
    private static readonly byte[] Step1Empty = [0, 0, 1, 0];
    private static readonly byte[] Step2Empty = [0, 1, 2, 0, 0];
    private readonly TestApp _app = new();

    public void Dispose() => _app.Dispose();

    private async Task<(HttpClient Client, JsonElement Session)> SessionAsync(string sub, string name, Guid fileId)
    {
        var c = _app.ClientFor(sub, name);
        var res = await c.PostAsJsonAsync(new Uri("/api/sessions", UriKind.Relative), new { fileId = fileId.ToString() });
        Assert.Equal(HttpStatusCode.Created, res.StatusCode);
        return (c, await res.Content.ReadFromJsonAsync<JsonElement>());
    }

    private Task<WsClient> ConnectAsync(JsonElement session) =>
        _app.ConnectAsync($"{session.GetProperty("collab").GetProperty("url").GetString()}&token={session.GetProperty("sessionToken").GetString()}");

    private async Task<Guid> SharedFileAsync()
    {
        var alice = _app.ClientFor("alice", "Alice");
        foreach (var u in new[] { "bob", "carol" })
        {
            await _app.ClientFor(u, u).GetAsync(new Uri("/api/me", UriKind.Relative));
        }

        var file = await Api.UploadAsync(alice, "Kế hoạch.docx", "v1"u8.ToArray());
        Assert.Equal(HttpStatusCode.OK, await Api.ShareAsync(alice, "files", file.Id(), "bob", "editor"));
        Assert.Equal(HttpStatusCode.OK, await Api.ShareAsync(alice, "files", file.Id(), "carol", "viewer"));
        return file.Id();
    }

    [Fact]
    public async Task SessionInfoHasTheDocumentedShape()
    {
        var fileId = await SharedFileAsync();
        var (_, s) = await SessionAsync("carol", "Carol", fileId);
        Assert.Equal("local", s.GetProperty("host").GetString());
        Assert.Equal("local", s.GetProperty("protocol").GetString());
        Assert.Equal("view", s.GetProperty("mode").GetString());
        Assert.False(s.GetProperty("permissions").GetProperty("edit").GetBoolean());
        Assert.True(s.GetProperty("permissions").GetProperty("view").GetBoolean());
        Assert.Equal("Kế hoạch.docx", s.GetProperty("file").GetProperty("name").GetString());
        Assert.Equal("1", s.GetProperty("file").GetProperty("version").GetString());
        Assert.Equal("Carol", s.GetProperty("user").GetProperty("name").GetString());
        Assert.StartsWith($"/collab/{fileId}?session=", s.GetProperty("collab").GetProperty("url").GetString(), StringComparison.Ordinal);
        Assert.Equal(45, s.GetProperty("autosaveSeconds").GetInt32());

        // Session API requires the secret.
        var id = s.GetProperty("sessionId").GetString();
        var anon = _app.CreateClient();
        Assert.Equal(HttpStatusCode.Unauthorized, (await anon.GetAsync(new Uri($"/api/sessions/{id}", UriKind.Relative))).StatusCode);
        anon.DefaultRequestHeaders.Add("X-Session-Token", s.GetProperty("sessionToken").GetString());
        var again = await anon.GetFromJsonAsync<JsonElement>(new Uri($"/api/sessions/{id}", UriKind.Relative));
        Assert.Equal(JsonValueKind.Null, again.GetProperty("sessionToken").ValueKind);
        Assert.Equal("v1"u8.ToArray(), await anon.GetByteArrayAsync(new Uri($"/api/sessions/{id}/contents", UriKind.Relative)));
    }

    [Fact]
    public async Task RelaysSyncUpdatesAndAwarenessBetweenClients()
    {
        var fileId = await SharedFileAsync();
        var (_, sa) = await SessionAsync("alice", "Alice", fileId);
        var (_, sb) = await SessionAsync("bob", "Bob", fileId);
        var (_, sc) = await SessionAsync("carol", "Carol", fileId);

        await using var a = await ConnectAsync(sa);
        var welcome = await a.WaitEventAsync("welcome");
        Assert.True(welcome.GetProperty("seed").GetBoolean());
        Assert.Equal("edit", welcome.GetProperty("mode").GetString());

        // Seeder: server answers step1 with an empty step2 and asks for the full state.
        await a.SendAsync(Step1Empty);
        await a.WaitBinaryAsync(Step2Empty);
        await a.WaitBinaryAsync(Step1Empty);
        await a.SendAsync(YProtocol.EncodeSync(YProtocol.SyncStep2, [1, 2, 3]));

        await using var b = await ConnectAsync(sb);
        Assert.False((await b.WaitEventAsync("welcome")).GetProperty("seed").GetBoolean());
        await b.SendAsync(Step1Empty);
        await b.WaitBinaryAsync([0, 1, 3, 1, 2, 3]);
        await a.WaitAsync(m => m.EventType == "collaborators-changed" && m.Json.GetProperty("collaborators").GetArrayLength() == 2, "2 collaborators");

        // Incremental update A → B.
        await a.SendAsync([0, 2, 2, 9, 9]);
        await b.WaitBinaryAsync([0, 2, 2, 9, 9]);

        // Awareness B → A (relayed verbatim).
        var awareness = YProtocol.EncodeAwareness([new AwarenessEntry(42, 1, "{\"user\":{\"name\":\"Bob\"}}")]);
        await b.SendAsync(awareness);
        await a.WaitBinaryAsync(awareness);

        // Viewer updates are dropped.
        await using var c = await ConnectAsync(sc);
        Assert.Equal("view", (await c.WaitEventAsync("welcome")).GetProperty("mode").GetString());
        await c.WaitBinaryAsync(awareness); // newcomer receives current awareness states
        await c.SendAsync([0, 2, 1, 7]);
        await b.SendAsync([0, 2, 1, 8]);
        await a.WaitBinaryAsync([0, 2, 1, 8]);
        Assert.DoesNotContain(a.Messages, m => m.Data.SequenceEqual(new byte[] { 0, 2, 1, 7 }));

        // Ping/pong control message.
        await a.SendJsonAsync(new { type = "ping" });
        await a.WaitEventAsync("pong");

        // B leaves: A sees the awareness removal (state "null") and 2 collaborators.
        await b.DisposeAsync();
        var removal = await a.WaitAsync(m => m.Type == WebSocketMessageType.Binary && m.Data[0] == 1, "awareness removal");
        var pos = 1;
        var entry = Assert.Single(YProtocol.DecodeAwareness(YProtocol.ReadVarUint8Array(removal.Data, ref pos)));
        Assert.Equal(("null", 42UL, 2UL), (entry.State, entry.ClientId, entry.Clock));
    }

    [Fact]
    public async Task CheckpointSaveCreatesVersionWithEditors()
    {
        var fileId = await SharedFileAsync();
        var (alice, sa) = await SessionAsync("alice", "Alice", fileId);
        var (_, sb) = await SessionAsync("bob", "Bob", fileId);
        await using var a = await ConnectAsync(sa);
        await using var b = await ConnectAsync(sb);
        await a.WaitEventAsync("welcome");
        await a.SendAsync([0, 2, 1, 5]);
        await b.WaitBinaryAsync([0, 2, 1, 5]);

        var bob = _app.CreateClient();
        bob.DefaultRequestHeaders.Add("X-Session-Token", sb.GetProperty("sessionToken").GetString());
        var res = await bob.PostAsync(new Uri($"/api/sessions/{sb.GetProperty("sessionId").GetString()}/checkpoint?save=true", UriKind.Relative), new ByteArrayContent("v2-docx"u8.ToArray()));
        var body = await res.Content.ReadFromJsonAsync<JsonElement>();
        Assert.Equal("saved", body.GetProperty("outcome").GetString());
        Assert.Equal("2", body.GetProperty("version").GetString());
        var saved = await a.WaitEventAsync("saved");
        Assert.Equal("Alice", saved.GetProperty("editors")[0].GetString());

        var versions = await alice.GetFromJsonAsync<JsonElement>(new Uri($"/api/files/{fileId}/versions", UriKind.Relative));
        Assert.Equal("Edited in BlueOffice by Alice", versions[0].GetProperty("note").GetString());
        Assert.Equal("v2-docx"u8.ToArray(), await alice.GetByteArrayAsync(new Uri($"/api/files/{fileId}/content", UriKind.Relative)));

        // Viewer cannot upload checkpoints.
        var (_, sc) = await SessionAsync("carol", "Carol", fileId);
        var carol = _app.CreateClient();
        carol.DefaultRequestHeaders.Add("X-Session-Token", sc.GetProperty("sessionToken").GetString());
        Assert.Equal(HttpStatusCode.Forbidden, (await carol.PostAsync(new Uri($"/api/sessions/{sc.GetProperty("sessionId").GetString()}/checkpoint", UriKind.Relative), new ByteArrayContent([1]))).StatusCode);
    }

    [Fact]
    public async Task UnsavedLogIsPersistedAndReplayedToTheNextSession()
    {
        var fileId = await SharedFileAsync();
        var (_, s1) = await SessionAsync("alice", "Alice", fileId);
        await using (var a = await ConnectAsync(s1))
        {
            await a.WaitEventAsync("welcome");
            await a.SendAsync(YProtocol.EncodeSync(YProtocol.SyncStep2, [5, 5]));
            await a.SendAsync([0, 2, 1, 6]);
            await a.SendJsonAsync(new { type = "ping" });
            await a.WaitEventAsync("pong");
        }

        // Room closes with unsaved changes → snapshot in the log store (database by default).
        var log = _app.Services.GetRequiredService<CollabLogStore>();
        Assert.Equal("Database", log.Kind);
        await Eventually(() => log.Store.ListAsync("collab/", CancellationToken.None).GetAwaiter().GetResult().Any(k => k.EndsWith("/snapshot.bin", StringComparison.Ordinal)));
        Assert.False(Directory.Exists(Path.Combine(_app.ObjectsRoot, "collab")));

        var (_, s2) = await SessionAsync("bob", "Bob", fileId);
        await using var b = await ConnectAsync(s2);
        Assert.False((await b.WaitEventAsync("welcome")).GetProperty("seed").GetBoolean());
        await b.SendAsync(Step1Empty);
        await b.WaitBinaryAsync([0, 1, 2, 5, 5]);
        await b.WaitBinaryAsync([0, 1, 1, 6]);
    }

    [Fact]
    public async Task DowngradedShareSwitchesConnectionToViewMode()
    {
        var fileId = await SharedFileAsync();
        var alice = _app.ClientFor("alice", "Alice");
        var (_, sb) = await SessionAsync("bob", "Bob", fileId);
        await using var b = await ConnectAsync(sb);
        Assert.Equal("edit", (await b.WaitEventAsync("welcome")).GetProperty("mode").GetString());

        Assert.Equal(HttpStatusCode.OK, await Api.ShareAsync(alice, "files", fileId, "bob", "viewer"));
        var bob = _app.CreateClient();
        bob.DefaultRequestHeaders.Add("X-Session-Token", sb.GetProperty("sessionToken").GetString());
        var info = await (await bob.PostAsJsonAsync(new Uri($"/api/sessions/{sb.GetProperty("sessionId").GetString()}/refresh", UriKind.Relative), new { }))
            .Content.ReadFromJsonAsync<JsonElement>();
        Assert.Equal("view", info.GetProperty("mode").GetString());
        var changed = await b.WaitEventAsync("mode-changed");
        Assert.Equal("view", changed.GetProperty("mode").GetString());
    }

    [Fact]
    public async Task DirectJwtConnectionAndBadTokens()
    {
        var fileId = await SharedFileAsync();
        await using var a = await _app.ConnectAsync($"/collab/{fileId}?access_token={TestApp.UserToken("alice", "Alice")}");
        Assert.Equal("edit", (await a.WaitEventAsync("welcome")).GetProperty("mode").GetString());

        await Assert.ThrowsAnyAsync<Exception>(() => _app.ConnectAsync($"/collab/{fileId}?session=nope&token=nope"));
        await Assert.ThrowsAnyAsync<Exception>(() => _app.ConnectAsync($"/collab/{fileId}?access_token={TestApp.UserToken("mallory", "M")}"));
    }

    internal static async Task Eventually(Func<bool> condition, int timeoutMs = 5000)
    {
        var deadline = DateTime.UtcNow.AddMilliseconds(timeoutMs);
        while (!condition())
        {
            if (DateTime.UtcNow > deadline)
            {
                throw new TimeoutException("Condition not met.");
            }

            await Task.Delay(50);
        }
    }
}
