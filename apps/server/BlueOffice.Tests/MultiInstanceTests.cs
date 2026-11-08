using System.Net;
using System.Net.Http.Json;
using System.Text.Json;
using BlueOffice.Api.Collab;
using Microsoft.Extensions.DependencyInjection;
using Xunit;

namespace BlueOffice.Tests;

/// <summary>
/// Two <c>api</c> instances over the same database, object store and Redis (docs/plan/phase-6.md "Nhiều instance"):
/// a room is hosted by one instance at a time (lease in Redis), the other one sends its clients away with close code
/// 4009, and session ids carry the instance that holds them.
/// </summary>
public sealed class MultiInstanceTests : IDisposable
{
    private readonly string _dir = Directory.CreateTempSubdirectory("bo-multi-").FullName;
    private readonly FakeRedis _redis = new(TimeProvider.System);
    private readonly TestApp _a;
    private readonly TestApp _b;

    public MultiInstanceTests()
    {
        _a = Instance("a");
        // The first instance creates the database before the second one starts.
        _a.CreateClient().Dispose();
        _b = Instance("b");
    }

    private TestApp Instance(string id) => new(
        settings: new Dictionary<string, string?>
        {
            ["Collab:Ownership"] = "Redis",
            ["Collab:InstanceId"] = id,
            ["Collab:LeaseTime"] = "00:00:05",
        },
        dataDir: _dir,
        redis: _redis);

    public void Dispose()
    {
        _a.Dispose();
        _b.Dispose();
        try
        {
            Directory.Delete(_dir, recursive: true);
        }
        catch (IOException)
        {
            // Best effort.
        }
    }

    private static async Task<JsonElement> SessionAsync(TestApp app, string sub, Guid fileId)
    {
        var res = await app.ClientFor(sub, sub).PostAsJsonAsync(new Uri("/api/sessions", UriKind.Relative), new { fileId = fileId.ToString() });
        Assert.Equal(HttpStatusCode.Created, res.StatusCode);
        return await res.Content.ReadFromJsonAsync<JsonElement>();
    }

    private static Task<WsClient> ConnectAsync(TestApp app, JsonElement session) =>
        app.ConnectAsync($"{session.GetProperty("collab").GetProperty("url").GetString()}&token={session.GetProperty("sessionToken").GetString()}");

    private async Task<Guid> SharedFileAsync()
    {
        var alice = _a.ClientFor("alice", "Alice");
        await _a.ClientFor("bob", "bob").GetAsync(new Uri("/api/me", UriKind.Relative));
        var file = await Api.UploadAsync(alice, "Kế hoạch.docx", "v1"u8.ToArray());
        Assert.Equal(HttpStatusCode.OK, await Api.ShareAsync(alice, "files", file.Id(), "bob", "editor"));
        return file.Id();
    }

    private static async Task WaitClosedAsync(WsClient c)
    {
        for (var i = 0; i < 250 && c.CloseStatus is null; i++)
        {
            await Task.Delay(20);
        }
    }

    [Fact]
    public async Task OneInstanceHostsARoomAndTheOtherSendsClientsThere()
    {
        var fileId = await SharedFileAsync();
        var sa = await SessionAsync(_a, "alice", fileId);
        var sb = await SessionAsync(_b, "bob", fileId);
        Assert.StartsWith("a~", sa.GetProperty("sessionId").GetString(), StringComparison.Ordinal);
        Assert.StartsWith("b~", sb.GetProperty("sessionId").GetString(), StringComparison.Ordinal);

        // A opens the room and holds its lease.
        await using (var a = await ConnectAsync(_a, sa))
        {
            Assert.True((await a.WaitEventAsync("welcome")).GetProperty("seed").GetBoolean());
            Assert.Equal("a", await _redis.StringGetAsync("blueoffice:room:" + _a.Services.GetRequiredService<RoomManager>().Rooms.Single().Key));

            // B does not open a second room for the same document: its client is sent away (4009, owner "a").
            await using var b = await ConnectAsync(_b, sb);
            await WaitClosedAsync(b);
            Assert.Equal(RoomOwnedElsewhereException.CloseCode, (int?)b.CloseStatus);
            Assert.Equal("a", b.CloseDescription);
            Assert.Empty(_b.Services.GetRequiredService<RoomManager>().Rooms);

            // The session API of a session held by A answers 421 on B (the proxy routed it wrongly).
            var id = sa.GetProperty("sessionId").GetString();
            var wrong = _b.CreateClient();
            wrong.DefaultRequestHeaders.Add("X-Session-Token", sa.GetProperty("sessionToken").GetString());
            Assert.Equal(HttpStatusCode.MisdirectedRequest, (await wrong.GetAsync(new Uri($"/api/sessions/{id}", UriKind.Relative))).StatusCode);
        }

        // When A's room closes the lease is released and B may host the document.
        var rooms = _a.Services.GetRequiredService<RoomManager>();
        for (var i = 0; i < 250 && rooms.Rooms.Count > 0; i++)
        {
            await Task.Delay(20);
        }

        Assert.Empty(rooms.Rooms);
        await using var b2 = await ConnectAsync(_b, sb);
        Assert.True((await b2.WaitEventAsync("welcome")).GetProperty("seed").GetBoolean());
        Assert.Equal("b", _redis.Keys.Where(k => k.StartsWith("blueoffice:room:", StringComparison.Ordinal)).Select(k => _redis.StringGetAsync(k).Result).Single(v => v is not null));
    }

    [Fact]
    public async Task ARoomWhoseLeaseIsLostSendsItsClientsAway()
    {
        var fileId = await SharedFileAsync();
        var sa = await SessionAsync(_a, "alice", fileId);
        await using var a = await ConnectAsync(_a, sa);
        await a.WaitEventAsync("welcome");
        var key = "blueoffice:room:" + _a.Services.GetRequiredService<RoomManager>().Rooms.Single().Key;

        // The lease is taken over (A was cut off from Redis long enough for it to expire).
        Assert.True(await _redis.DeleteIfValueAsync(key, "a"));
        Assert.True(await _redis.StringSetIfNotExistsAsync(key, "b", TimeSpan.FromSeconds(30)));

        await _a.Services.GetRequiredService<CollabMaintenanceService>().TickAsync(CancellationToken.None);
        await WaitClosedAsync(a);
        Assert.Equal(RoomOwnedElsewhereException.CloseCode, (int?)a.CloseStatus);
        // A never frees a lease it does not hold.
        Assert.Equal("b", await _redis.StringGetAsync(key));
    }

    [Fact]
    public async Task LeasesAreTakenRenewedAndReleasedOnlyByTheirOwner()
    {
        var opts = new InstanceOptions { InstanceId = "x", LeaseTime = TimeSpan.FromSeconds(10) };
        var x = new RedisRoomOwnership(_redis, opts);
        var y = new RedisRoomOwnership(_redis, new InstanceOptions { InstanceId = "y", LeaseTime = TimeSpan.FromSeconds(10) });
        Assert.Null(await x.AcquireAsync("r"));
        Assert.Null(await x.AcquireAsync("r"));
        Assert.Equal("x", await y.AcquireAsync("r"));
        Assert.False(await y.RenewAsync("r"));
        await y.ReleaseAsync("r");
        Assert.True(await x.RenewAsync("r"));
        await x.ReleaseAsync("r");
        Assert.Null(await y.AcquireAsync("r"));
    }
}
