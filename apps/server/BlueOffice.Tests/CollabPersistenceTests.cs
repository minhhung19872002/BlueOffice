using System.Net;
using BlueOffice.Api.Collab;
using BlueOffice.Domain;
using BlueOffice.Infrastructure.Data;
using Microsoft.Data.Sqlite;
using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.DependencyInjection;
using Xunit;

namespace BlueOffice.Tests;

/// <summary>The collaboration log lives in the database and survives an api restart (P6.5).</summary>
public sealed class CollabPersistenceTests
{
    private static async Task InTempDirAsync(string prefix, Func<string, Task> body)
    {
        var dir = Directory.CreateTempSubdirectory(prefix).FullName;
        try
        {
            await body(dir);
        }
        finally
        {
            SqliteConnection.ClearAllPools();
            try
            {
                Directory.Delete(dir, recursive: true);
            }
            catch (IOException)
            {
                // Best effort.
            }
        }
    }

    [Fact]
    public Task UnsavedLogSurvivesAnApiRestart() => InTempDirAsync("bo-restart-", async dir =>
    {
        Guid fileId;
        using (var first = new TestApp(dataDir: dir))
        {
            var alice = first.ClientFor("alice", "Alice");
            await first.ClientFor("bob", "Bob").GetAsync(new Uri("/api/me", UriKind.Relative));
            var file = await Api.UploadAsync(alice, "Biên bản.docx", "v1"u8.ToArray());
            fileId = file.Id();
            Assert.Equal(HttpStatusCode.OK, await Api.ShareAsync(alice, "files", fileId, "bob", "editor"));
            var log = first.Services.GetRequiredService<CollabLogStore>().Store;
            await using (var a = await first.ConnectAsync($"/collab/{fileId}?access_token={TestApp.UserToken("alice", "Alice")}"))
            {
                await a.WaitEventAsync("welcome");
                await a.SendAsync(YProtocol.EncodeSync(YProtocol.SyncStep2, [7, 7]));
                await a.SendAsync([0, 2, 1, 8]);
                await a.SendJsonAsync(new { type = "ping" });
                await a.WaitEventAsync("pong");
            }

            await CollabTests.Eventually(() => log.ListAsync("collab/", CancellationToken.None).GetAwaiter().GetResult().Any(k => k.EndsWith("/snapshot.bin", StringComparison.Ordinal)));
        }

        // The object store holds no log: the replay below can only come from the database.
        Assert.False(Directory.Exists(Path.Combine(dir, "objects", "collab")));

        using var second = new TestApp(dataDir: dir);
        await using var b = await second.ConnectAsync($"/collab/{fileId}?access_token={TestApp.UserToken("bob", "Bob")}");
        Assert.False((await b.WaitEventAsync("welcome")).GetProperty("seed").GetBoolean());
        await b.SendAsync([0, 0, 1, 0]);
        await b.WaitBinaryAsync([0, 1, 2, 7, 7]);
        await b.WaitBinaryAsync([0, 1, 1, 8]);
    });

    [Fact]
    public Task SchemaUpgradeAddsTheLogTableToAnExistingDatabase() => InTempDirAsync("bo-upgrade-", async dir =>
    {
        // A database created (by EnsureCreated, so without migration history) before the CollabLog table existed.
        using (var first = new TestApp(dataDir: dir))
        {
            await first.ClientFor("alice", "Alice").GetAsync(new Uri("/api/me", UriKind.Relative));
            using var scope = first.Services.CreateScope();
            var db = scope.ServiceProvider.GetRequiredService<BlueOfficeDbContext>().Database;
            await db.ExecuteSqlRawAsync("DROP TABLE \"CollabLog\"");
            await db.ExecuteSqlRawAsync("DROP TABLE \"__EFMigrationsHistory\"");
            await db.ExecuteSqlRawAsync("DROP TABLE \"ScriptSchedules\"");
            await db.ExecuteSqlRawAsync("DROP TABLE \"ScriptRuns\"");
            await db.ExecuteSqlRawAsync("DROP TABLE \"AddinInstalls\"");
            await db.ExecuteSqlRawAsync("DROP TABLE \"Addins\"");
        }

        using var second = new TestApp(dataDir: dir);
        using (var scope = second.Services.CreateScope())
        {
            Assert.Equal(0, await scope.ServiceProvider.GetRequiredService<BlueOfficeDbContext>().CollabLog.CountAsync());
        }

        var store = second.Services.GetRequiredService<CollabLogStore>().Store;
        await store.PutAsync("collab/abc/u-0000000001.bin", new MemoryStream([1, 2]), "application/octet-stream", CancellationToken.None);
        await store.PutAsync("collab/abc/meta.json", new MemoryStream("{}"u8.ToArray()), "application/json", CancellationToken.None);
        await store.PutAsync("collab/abd/snapshot.bin", new MemoryStream([3]), "application/octet-stream", CancellationToken.None);
        await store.PutAsync("collab/abc/meta.json", new MemoryStream("{\"v\":1}"u8.ToArray()), "application/json", CancellationToken.None);
        Assert.Equal(["collab/abc/meta.json", "collab/abc/u-0000000001.bin"], await store.ListAsync("collab/abc/", CancellationToken.None));
        Assert.Equal(["collab/abc/u-0000000001.bin"], await store.ListAsync("collab/abc/u-", CancellationToken.None));
        Assert.Equal(3, (await store.ListAsync("collab/", CancellationToken.None)).Count);
        await using (var s = await store.OpenReadAsync("collab/abc/meta.json", CancellationToken.None))
        {
            Assert.Equal(7, s.Length);
        }

        await store.DeleteAsync("collab/abc/meta.json", CancellationToken.None);
        Assert.False(await store.ExistsAsync("collab/abc/meta.json", CancellationToken.None));
        await Assert.ThrowsAsync<ObjectNotFoundException>(() => store.OpenReadAsync("collab/abc/meta.json", CancellationToken.None));
    });

    [Fact]
    public void ObjectStoreCanStillBeSelected()
    {
        using var app = new TestApp(settings: new() { ["Collab:SnapshotStore"] = "ObjectStore" });
        Assert.Equal("ObjectStore", app.Services.GetRequiredService<CollabLogStore>().Kind);
    }
}
