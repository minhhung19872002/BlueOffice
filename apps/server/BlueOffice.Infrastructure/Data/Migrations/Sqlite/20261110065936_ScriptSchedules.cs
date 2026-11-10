using System;
using Microsoft.EntityFrameworkCore.Migrations;

#nullable disable

namespace BlueOffice.Infrastructure.Data.Migrations.Sqlite
{
    /// <inheritdoc />
    public partial class ScriptSchedules : Migration
    {
        /// <inheritdoc />
        protected override void Up(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.CreateTable(
                name: "ScriptSchedules",
                columns: table => new
                {
                    Id = table.Column<string>(type: "TEXT", maxLength: 32, nullable: false),
                    Host = table.Column<string>(type: "TEXT", maxLength: 64, nullable: false),
                    FileId = table.Column<string>(type: "TEXT", maxLength: 256, nullable: false),
                    FileName = table.Column<string>(type: "TEXT", maxLength: 255, nullable: false),
                    UserId = table.Column<string>(type: "TEXT", maxLength: 256, nullable: false),
                    UserName = table.Column<string>(type: "TEXT", maxLength: 256, nullable: false),
                    Name = table.Column<string>(type: "TEXT", maxLength: 200, nullable: false),
                    Code = table.Column<string>(type: "TEXT", nullable: false),
                    Cron = table.Column<string>(type: "TEXT", maxLength: 120, nullable: false),
                    Enabled = table.Column<bool>(type: "INTEGER", nullable: false),
                    NextRunAt = table.Column<DateTime>(type: "TEXT", nullable: true),
                    LastRunAt = table.Column<DateTime>(type: "TEXT", nullable: true),
                    LastStatus = table.Column<string>(type: "TEXT", maxLength: 16, nullable: true),
                    LastError = table.Column<string>(type: "TEXT", maxLength: 2000, nullable: true),
                    CreatedAt = table.Column<DateTime>(type: "TEXT", nullable: false)
                },
                constraints: table =>
                {
                    table.PrimaryKey("PK_ScriptSchedules", x => x.Id);
                });

            migrationBuilder.CreateIndex(
                name: "IX_ScriptSchedules_Enabled_NextRunAt",
                table: "ScriptSchedules",
                columns: new[] { "Enabled", "NextRunAt" });

            migrationBuilder.CreateIndex(
                name: "IX_ScriptSchedules_Host_FileId_UserId",
                table: "ScriptSchedules",
                columns: new[] { "Host", "FileId", "UserId" });
        }

        /// <inheritdoc />
        protected override void Down(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.DropTable(
                name: "ScriptSchedules");
        }
    }
}
