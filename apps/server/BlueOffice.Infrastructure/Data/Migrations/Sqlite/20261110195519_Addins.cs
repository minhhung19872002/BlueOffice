using System;
using Microsoft.EntityFrameworkCore.Migrations;

#nullable disable

namespace BlueOffice.Infrastructure.Data.Migrations.Sqlite
{
    /// <inheritdoc />
    public partial class Addins : Migration
    {
        /// <inheritdoc />
        protected override void Up(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.CreateTable(
                name: "AddinInstalls",
                columns: table => new
                {
                    Id = table.Column<string>(type: "TEXT", maxLength: 32, nullable: false),
                    AddinId = table.Column<string>(type: "TEXT", maxLength: 128, nullable: false),
                    UserKey = table.Column<string>(type: "TEXT", maxLength: 600, nullable: false),
                    Permissions = table.Column<string>(type: "TEXT", maxLength: 4000, nullable: false),
                    Version = table.Column<string>(type: "TEXT", maxLength: 64, nullable: false),
                    InstalledAt = table.Column<DateTime>(type: "TEXT", nullable: false)
                },
                constraints: table =>
                {
                    table.PrimaryKey("PK_AddinInstalls", x => x.Id);
                });

            migrationBuilder.CreateTable(
                name: "Addins",
                columns: table => new
                {
                    Id = table.Column<string>(type: "TEXT", maxLength: 128, nullable: false),
                    Version = table.Column<string>(type: "TEXT", maxLength: 64, nullable: false),
                    Name = table.Column<string>(type: "TEXT", maxLength: 100, nullable: false),
                    Apps = table.Column<string>(type: "TEXT", maxLength: 32, nullable: false),
                    Manifest = table.Column<string>(type: "TEXT", nullable: false),
                    Trusted = table.Column<bool>(type: "INTEGER", nullable: false),
                    Enabled = table.Column<bool>(type: "INTEGER", nullable: false),
                    PublishedBy = table.Column<string>(type: "TEXT", maxLength: 320, nullable: false),
                    CreatedAt = table.Column<DateTime>(type: "TEXT", nullable: false),
                    UpdatedAt = table.Column<DateTime>(type: "TEXT", nullable: false)
                },
                constraints: table =>
                {
                    table.PrimaryKey("PK_Addins", x => x.Id);
                });

            migrationBuilder.CreateIndex(
                name: "IX_AddinInstalls_AddinId_UserKey",
                table: "AddinInstalls",
                columns: new[] { "AddinId", "UserKey" },
                unique: true);

            migrationBuilder.CreateIndex(
                name: "IX_AddinInstalls_UserKey",
                table: "AddinInstalls",
                column: "UserKey");
        }

        /// <inheritdoc />
        protected override void Down(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.DropTable(
                name: "AddinInstalls");

            migrationBuilder.DropTable(
                name: "Addins");
        }
    }
}
