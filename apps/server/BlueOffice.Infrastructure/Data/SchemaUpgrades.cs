using Microsoft.EntityFrameworkCore;

namespace BlueOffice.Infrastructure.Data;

/// <summary>
/// Additive schema changes for databases created before a table existed. <c>EnsureCreated</c> only creates the schema
/// of an empty database, so tables added later are created here with <c>CREATE TABLE IF NOT EXISTS</c> (SQLite and
/// PostgreSQL). Runs at startup after <c>EnsureCreated</c> (see docs/deploy/upgrade.md).
/// </summary>
public static class SchemaUpgrades
{
    /// <summary>Table of <see cref="Storage.CollabLogEntry"/>.</summary>
    public const string CollabLogTable = "CollabLog";

    /// <summary>Creates the tables missing from an existing database.</summary>
    public static async Task ApplyAsync(BlueOfficeDbContext db, CancellationToken ct = default)
    {
        string[] statements = db.Database.ProviderName?.Contains("Npgsql", StringComparison.OrdinalIgnoreCase) == true
            ?
            [
                """
                CREATE TABLE IF NOT EXISTS "CollabLog" (
                    "Key" character varying(512) NOT NULL PRIMARY KEY,
                    "Room" character varying(512) NOT NULL,
                    "Data" bytea NOT NULL,
                    "ContentType" character varying(128) NOT NULL,
                    "UpdatedAt" timestamp with time zone NOT NULL)
                """,
                """CREATE INDEX IF NOT EXISTS "IX_CollabLog_Room" ON "CollabLog" ("Room")""",
            ]
            :
            [
                """
                CREATE TABLE IF NOT EXISTS "CollabLog" (
                    "Key" TEXT NOT NULL CONSTRAINT "PK_CollabLog" PRIMARY KEY,
                    "Room" TEXT NOT NULL,
                    "Data" BLOB NOT NULL,
                    "ContentType" TEXT NOT NULL,
                    "UpdatedAt" TEXT NOT NULL)
                """,
                """CREATE INDEX IF NOT EXISTS "IX_CollabLog_Room" ON "CollabLog" ("Room")""",
            ];
        foreach (var sql in statements)
        {
            await db.Database.ExecuteSqlRawAsync(sql, ct);
        }
    }
}
