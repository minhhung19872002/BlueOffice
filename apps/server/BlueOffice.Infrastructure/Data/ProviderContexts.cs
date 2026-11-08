using Microsoft.EntityFrameworkCore;
using Microsoft.EntityFrameworkCore.Design;

namespace BlueOffice.Infrastructure.Data;

/// <summary>SQLite context (dev, tests, single-machine installs); migrations in <c>Data/Migrations/Sqlite</c>.</summary>
public sealed class SqliteBlueOfficeDbContext(DbContextOptions<SqliteBlueOfficeDbContext> options) : BlueOfficeDbContext(options);

/// <summary>PostgreSQL context (production); migrations in <c>Data/Migrations/Postgres</c>.</summary>
public sealed class PostgresBlueOfficeDbContext(DbContextOptions<PostgresBlueOfficeDbContext> options) : BlueOfficeDbContext(options);

/// <summary>Design-time factory for <c>dotnet ef migrations add … --context SqliteBlueOfficeDbContext</c>.</summary>
public sealed class SqliteDesignTimeFactory : IDesignTimeDbContextFactory<SqliteBlueOfficeDbContext>
{
    /// <inheritdoc />
    public SqliteBlueOfficeDbContext CreateDbContext(string[] args) =>
        new(new DbContextOptionsBuilder<SqliteBlueOfficeDbContext>().UseSqlite("Data Source=design-time.db").Options);
}

/// <summary>Design-time factory for <c>dotnet ef migrations add … --context PostgresBlueOfficeDbContext</c> (no connection is opened).</summary>
public sealed class PostgresDesignTimeFactory : IDesignTimeDbContextFactory<PostgresBlueOfficeDbContext>
{
    /// <inheritdoc />
    public PostgresBlueOfficeDbContext CreateDbContext(string[] args) =>
        new(new DbContextOptionsBuilder<PostgresBlueOfficeDbContext>().UseNpgsql("Host=localhost;Database=design_time").Options);
}
