using System;
using Microsoft.EntityFrameworkCore.Migrations;

#nullable disable

namespace BlueOffice.Infrastructure.Data.Migrations.Postgres
{
    /// <inheritdoc />
    public partial class ScriptRuns : Migration
    {
        /// <inheritdoc />
        protected override void Up(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.AddColumn<int>(
                name: "ConsecutiveFailures",
                table: "ScriptSchedules",
                type: "integer",
                nullable: false,
                defaultValue: 0);

            migrationBuilder.AddColumn<string>(
                name: "DisabledReason",
                table: "ScriptSchedules",
                type: "character varying(500)",
                maxLength: 500,
                nullable: true);

            migrationBuilder.AddColumn<string>(
                name: "ProtectedToken",
                table: "ScriptSchedules",
                type: "character varying(16000)",
                maxLength: 16000,
                nullable: true);

            migrationBuilder.CreateTable(
                name: "ScriptRuns",
                columns: table => new
                {
                    Id = table.Column<Guid>(type: "uuid", nullable: false),
                    ScheduleId = table.Column<string>(type: "character varying(32)", maxLength: 32, nullable: false),
                    StartedAt = table.Column<DateTime>(type: "timestamp with time zone", nullable: false),
                    DurationMs = table.Column<long>(type: "bigint", nullable: false),
                    Status = table.Column<string>(type: "character varying(16)", maxLength: 16, nullable: false),
                    Error = table.Column<string>(type: "character varying(2000)", maxLength: 2000, nullable: true),
                    Logs = table.Column<string>(type: "character varying(8000)", maxLength: 8000, nullable: true),
                    Version = table.Column<string>(type: "character varying(256)", maxLength: 256, nullable: true)
                },
                constraints: table =>
                {
                    table.PrimaryKey("PK_ScriptRuns", x => x.Id);
                });

            migrationBuilder.CreateIndex(
                name: "IX_ScriptRuns_ScheduleId_StartedAt",
                table: "ScriptRuns",
                columns: new[] { "ScheduleId", "StartedAt" });
        }

        /// <inheritdoc />
        protected override void Down(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.DropTable(
                name: "ScriptRuns");

            migrationBuilder.DropColumn(
                name: "ConsecutiveFailures",
                table: "ScriptSchedules");

            migrationBuilder.DropColumn(
                name: "DisabledReason",
                table: "ScriptSchedules");

            migrationBuilder.DropColumn(
                name: "ProtectedToken",
                table: "ScriptSchedules");
        }
    }
}
