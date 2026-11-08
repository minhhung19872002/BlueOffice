using Microsoft.EntityFrameworkCore;
using Microsoft.EntityFrameworkCore.Infrastructure;
using Microsoft.EntityFrameworkCore.Migrations;
using Microsoft.EntityFrameworkCore.Storage;

namespace BlueOffice.Infrastructure.Data;

/// <summary>
/// Brings the database schema up to date with EF Core migrations (<c>Data/Migrations/Sqlite</c> or
/// <c>Data/Migrations/Postgres</c> depending on the provider). A database created by <c>EnsureCreated</c> before
/// migrations existed (BlueOffice ≤ 0.6, no <c>__EFMigrationsHistory</c>) is adopted: its tables added later are
/// created (<see cref="SchemaUpgrades"/>), the first migration is recorded as applied, and later migrations run on it.
/// </summary>
public static class DatabaseMigrator
{
    /// <summary>Migrates <paramref name="db"/> to the latest migration.</summary>
    public static async Task MigrateAsync(BlueOfficeDbContext db, CancellationToken ct = default)
    {
        var creator = db.GetService<IRelationalDatabaseCreator>();
        var history = db.GetService<IHistoryRepository>();
        if (await creator.ExistsAsync(ct) && await creator.HasTablesAsync(ct) && !await history.ExistsAsync(ct))
        {
            await SchemaUpgrades.ApplyAsync(db, ct);
            var initial = db.Database.GetMigrations().First();
            await db.Database.ExecuteSqlRawAsync(history.GetCreateIfNotExistsScript(), ct);
            await db.Database.ExecuteSqlRawAsync(history.GetInsertScript(new HistoryRow(initial, ProductInfo.GetVersion())), ct);
        }

        await db.Database.MigrateAsync(ct);
    }
}
