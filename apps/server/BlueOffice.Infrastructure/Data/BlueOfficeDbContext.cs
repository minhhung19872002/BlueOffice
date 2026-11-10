using BlueOffice.Domain;
using Microsoft.EntityFrameworkCore;
using Microsoft.EntityFrameworkCore.Storage.ValueConversion;

namespace BlueOffice.Infrastructure.Data;

/// <summary>
/// EF Core context (PostgreSQL in production, SQLite for dev/tests). Each provider has its own subclass
/// (<see cref="SqliteBlueOfficeDbContext"/>, <see cref="PostgresBlueOfficeDbContext"/>) so that each gets its own
/// migrations (<c>Data/Migrations/Sqlite</c>, <c>Data/Migrations/Postgres</c>); services resolve this base type.
/// </summary>
public abstract class BlueOfficeDbContext(DbContextOptions options) : DbContext(options)
{
    /// <summary>Users.</summary>
    public DbSet<User> Users => Set<User>();

    /// <summary>Folders.</summary>
    public DbSet<Folder> Folders => Set<Folder>();

    /// <summary>Files.</summary>
    public DbSet<FileItem> Files => Set<FileItem>();

    /// <summary>File versions.</summary>
    public DbSet<FileVersion> FileVersions => Set<FileVersion>();

    /// <summary>Shares and links.</summary>
    public DbSet<Share> Shares => Set<Share>();

    /// <summary>Audit log.</summary>
    public DbSet<AuditEntry> AuditEntries => Set<AuditEntry>();

    /// <summary>Collaboration sessions.</summary>
    public DbSet<CollabSession> CollabSessions => Set<CollabSession>();

    /// <summary>Collaboration update log (snapshots + incremental batches), see <see cref="Storage.DatabaseObjectStore"/>.</summary>
    public DbSet<Storage.CollabLogEntry> CollabLog => Set<Storage.CollabLogEntry>();

    /// <summary>Scheduled BlueScript runs (CORE-SCRIPT-002).</summary>
    public DbSet<ScriptSchedule> ScriptSchedules => Set<ScriptSchedule>();

    /// <inheritdoc />
    protected override void OnModelCreating(ModelBuilder modelBuilder)
    {
        modelBuilder.Entity<User>(e =>
        {
            e.HasIndex(x => x.Subject).IsUnique();
            e.Property(x => x.Subject).HasMaxLength(256);
            e.Property(x => x.DisplayName).HasMaxLength(256);
            e.Property(x => x.Email).HasMaxLength(320);
        });
        modelBuilder.Entity<Folder>(e =>
        {
            e.HasIndex(x => new { x.OwnerId, x.ParentId });
            e.HasIndex(x => x.ParentId);
            e.Property(x => x.Name).HasMaxLength(255);
        });
        modelBuilder.Entity<FileItem>(e =>
        {
            e.HasIndex(x => x.FolderId);
            e.HasIndex(x => x.OwnerId);
            e.Property(x => x.Name).HasMaxLength(255);
            e.Property(x => x.ContentType).HasMaxLength(128);
        });
        modelBuilder.Entity<FileVersion>(e =>
        {
            e.HasIndex(x => new { x.FileId, x.Number }).IsUnique();
            e.Property(x => x.ObjectKey).HasMaxLength(512);
            e.Property(x => x.Sha256).HasMaxLength(64);
        });
        modelBuilder.Entity<Share>(e =>
        {
            e.HasIndex(x => new { x.ResourceKind, x.ResourceId });
            e.HasIndex(x => x.GranteeUserId);
            e.HasIndex(x => x.LinkToken).IsUnique();
            e.Property(x => x.ResourceKind).HasConversion<string>().HasMaxLength(16);
            e.Property(x => x.Role).HasConversion<string>().HasMaxLength(16);
            e.Property(x => x.LinkToken).HasMaxLength(64);
        });
        modelBuilder.Entity<AuditEntry>(e =>
        {
            e.HasIndex(x => new { x.Host, x.ResourceId, x.At });
            e.Property(x => x.Action).HasConversion<string>().HasMaxLength(16);
            e.Property(x => x.ResourceKind).HasConversion<string>().HasMaxLength(16);
            e.Property(x => x.Actor).HasMaxLength(256);
            e.Property(x => x.Host).HasMaxLength(64);
            e.Property(x => x.ResourceId).HasMaxLength(256);
        });
        modelBuilder.Entity<CollabSession>(e =>
        {
            e.HasIndex(x => x.RoomKey);
            e.Property(x => x.RoomKey).HasMaxLength(320);
            e.Property(x => x.FileId).HasMaxLength(256);
        });

        modelBuilder.Entity<ScriptSchedule>(e =>
        {
            e.HasIndex(x => new { x.Host, x.FileId, x.UserId });
            e.HasIndex(x => new { x.Enabled, x.NextRunAt });
            e.Property(x => x.Id).HasMaxLength(32);
            e.Property(x => x.Host).HasMaxLength(64);
            e.Property(x => x.FileId).HasMaxLength(256);
            e.Property(x => x.FileName).HasMaxLength(255);
            e.Property(x => x.UserId).HasMaxLength(256);
            e.Property(x => x.UserName).HasMaxLength(256);
            e.Property(x => x.Name).HasMaxLength(200);
            e.Property(x => x.Cron).HasMaxLength(120);
            e.Property(x => x.LastStatus).HasMaxLength(16);
            e.Property(x => x.LastError).HasMaxLength(2000);
        });

        modelBuilder.Entity<Storage.CollabLogEntry>(e =>
        {
            e.ToTable(SchemaUpgrades.CollabLogTable);
            e.HasKey(x => x.Key);
            e.Property(x => x.Key).HasMaxLength(512);
            e.Property(x => x.Room).HasMaxLength(512);
            e.Property(x => x.ContentType).HasMaxLength(128);
            e.HasIndex(x => x.Room).HasDatabaseName("IX_CollabLog_Room");
        });

        // All timestamps are UTC; SQLite returns DateTimeKind.Unspecified, so restore the kind on read.
        var utc = new ValueConverter<DateTime, DateTime>(v => v, v => DateTime.SpecifyKind(v, DateTimeKind.Utc));
        var utcNullable = new ValueConverter<DateTime?, DateTime?>(v => v, v => v.HasValue ? DateTime.SpecifyKind(v.Value, DateTimeKind.Utc) : v);
        foreach (var p in modelBuilder.Model.GetEntityTypes().SelectMany(t => t.GetProperties()))
        {
            if (p.ClrType == typeof(DateTime))
            {
                p.SetValueConverter(utc);
            }
            else if (p.ClrType == typeof(DateTime?))
            {
                p.SetValueConverter(utcNullable);
            }
        }
    }
}
