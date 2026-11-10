using BlueOffice.Domain;
using BlueOffice.Infrastructure.Data;
using Microsoft.Data.Sqlite;
using Microsoft.EntityFrameworkCore;
using Microsoft.EntityFrameworkCore.Infrastructure;
using Microsoft.EntityFrameworkCore.Metadata;
using Microsoft.EntityFrameworkCore.Migrations;
using Npgsql;
using Xunit;

namespace BlueOffice.Tests;

/// <summary>EF Core migrations (SQLite and PostgreSQL) and the adoption of databases made by EnsureCreated.</summary>
public sealed class MigrationTests
{
    private static async Task WithSqliteAsync(Func<Func<SqliteBlueOfficeDbContext>, Task> body)
    {
        var dir = Directory.CreateTempSubdirectory("bo-migrate-").FullName;
        var options = new DbContextOptionsBuilder<SqliteBlueOfficeDbContext>()
            .UseSqlite($"Data Source={Path.Combine(dir, "db.sqlite")}")
            .Options;
        try
        {
            await body(() => new SqliteBlueOfficeDbContext(options));
        }
        finally
        {
            SqliteConnection.ClearAllPools();
            Directory.Delete(dir, recursive: true);
        }
    }

    [Fact]
    public Task FreshDatabaseIsCreatedByMigrations() => WithSqliteAsync(async make =>
    {
        await using var db = make();
        await DatabaseMigrator.MigrateAsync(db);
        Assert.Equal(db.Database.GetMigrations(), await db.Database.GetAppliedMigrationsAsync());
        Assert.Empty(await db.Database.GetPendingMigrationsAsync());
        Assert.Equal(0, await db.CollabLog.CountAsync());
    });

    [Fact]
    public Task DatabaseMadeByEnsureCreatedIsAdoptedWithItsData() => WithSqliteAsync(async make =>
    {
        // BlueOffice ≤ 0.6: EnsureCreated, then a table added later (CollabLog) missing.
        await using (var old = make())
        {
            await old.Database.EnsureCreatedAsync();
            await old.Database.ExecuteSqlRawAsync("DROP TABLE \"CollabLog\"");
            // Tables of later migrations (ScriptSchedules, ScriptRuns) did not exist either.
            await old.Database.ExecuteSqlRawAsync("DROP TABLE \"ScriptSchedules\"");
            await old.Database.ExecuteSqlRawAsync("DROP TABLE \"ScriptRuns\"");
            old.Users.Add(new User { Subject = "dev|alice", DisplayName = "Alice" });
            await old.SaveChangesAsync();
        }

        await using (var db = make())
        {
            await DatabaseMigrator.MigrateAsync(db);
            Assert.Equal(db.Database.GetMigrations(), await db.Database.GetAppliedMigrationsAsync());
            Assert.Equal("Alice", (await db.Users.SingleAsync()).DisplayName);
            Assert.Equal(0, await db.CollabLog.CountAsync());
        }

        // Starting again changes nothing.
        await using (var db = make())
        {
            await DatabaseMigrator.MigrateAsync(db);
            Assert.Single(await db.Users.ToListAsync());
        }
    });

    /// <summary>The migrations' snapshot matches the model: a model change without a new migration fails here
    /// (add one with <c>dotnet ef migrations add &lt;Name&gt; --context SqliteBlueOfficeDbContext</c> and
    /// <c>PostgresBlueOfficeDbContext</c>, see docs/deploy/upgrade.md).</summary>
    [Fact]
    public void MigrationsAreUpToDateWithTheModel()
    {
        using var sqlite = new SqliteDesignTimeFactory().CreateDbContext([]);
        using var postgres = new PostgresDesignTimeFactory().CreateDbContext([]);
        foreach (DbContext db in new DbContext[] { sqlite, postgres })
        {
#pragma warning disable EF1001 // internal model-diff API, as used by `dotnet ef migrations has-pending-model-changes` (EF 9)
            var snapshot = db.GetService<IMigrationsAssembly>().ModelSnapshot?.Model;
            Assert.NotNull(snapshot);
            if (snapshot is IMutableModel mutable)
            {
                snapshot = mutable.FinalizeModel();
            }

            snapshot = db.GetService<IModelRuntimeInitializer>().Initialize(snapshot);
            var current = db.GetService<IDesignTimeModel>().Model;
            var differences = db.GetService<IMigrationsModelDiffer>()
                .GetDifferences(snapshot.GetRelationalModel(), current.GetRelationalModel());
#pragma warning restore EF1001
            Assert.True(differences.Count == 0, $"{db.GetType().Name}: {string.Join(", ", differences.Select(d => d.GetType().Name))}");
        }
    }

    /// <summary>The same on a real PostgreSQL server when <c>BO_TEST_POSTGRES</c> holds a connection string of a
    /// role that may create databases (e.g. <c>Host=localhost;Username=bo;Password=bo;Database=bo</c>); a temporary
    /// database is created and dropped. Without the variable the test does nothing.</summary>
    [Fact]
    public async Task PostgresMigrationsAndAdoptionWhenConfigured()
    {
        var admin = Environment.GetEnvironmentVariable("BO_TEST_POSTGRES");
        if (string.IsNullOrEmpty(admin))
        {
            return;
        }

        var name = $"bo_migrate_{Guid.NewGuid():N}";
        await using (var c = new NpgsqlConnection(admin))
        {
            await c.OpenAsync();
            await using var cmd = new NpgsqlCommand($"CREATE DATABASE \"{name}\"", c);
            await cmd.ExecuteNonQueryAsync();
        }

        var cs = new NpgsqlConnectionStringBuilder(admin) { Database = name, Pooling = false }.ConnectionString;
        var options = new DbContextOptionsBuilder<PostgresBlueOfficeDbContext>().UseNpgsql(cs).Options;
        try
        {
            // Fresh database.
            await using (var db = new PostgresBlueOfficeDbContext(options))
            {
                await DatabaseMigrator.MigrateAsync(db);
                Assert.Equal(db.Database.GetMigrations(), await db.Database.GetAppliedMigrationsAsync());
                db.Users.Add(new User { Subject = "dev|alice", DisplayName = "Alice" });
                await db.SaveChangesAsync();
                // Back to a ≤ 0.6 database: no history, no CollabLog.
                await db.Database.ExecuteSqlRawAsync("DROP TABLE \"__EFMigrationsHistory\"");
                await db.Database.ExecuteSqlRawAsync("DROP TABLE \"CollabLog\"");
            }

            await using (var db = new PostgresBlueOfficeDbContext(options))
            {
                await DatabaseMigrator.MigrateAsync(db);
                Assert.Equal(db.Database.GetMigrations(), await db.Database.GetAppliedMigrationsAsync());
                Assert.Equal("Alice", (await db.Users.SingleAsync()).DisplayName);
                Assert.Equal(0, await db.CollabLog.CountAsync());
            }
        }
        finally
        {
            await using var c = new NpgsqlConnection(admin);
            await c.OpenAsync();
            await using var cmd = new NpgsqlCommand($"DROP DATABASE IF EXISTS \"{name}\" WITH (FORCE)", c);
            await cmd.ExecuteNonQueryAsync();
        }
    }
}
