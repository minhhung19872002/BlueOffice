using System.Text.Json;
using System.Text.Json.Serialization;
using BlueOffice.Api.Audit;
using BlueOffice.Api.Auth;
using BlueOffice.Api.Collab;
using BlueOffice.Api.Common;
using BlueOffice.Api.Files;
using BlueOffice.Api.Jobs;
using BlueOffice.Api.Sessions;
using BlueOffice.Api.Signing;
using BlueOffice.Domain;
using BlueOffice.Infrastructure;
using BlueOffice.Infrastructure.Data;
using BlueOffice.Infrastructure.Hosts;
using BlueOffice.Infrastructure.Storage;
using Microsoft.AspNetCore.Diagnostics.HealthChecks;
using Microsoft.AspNetCore.Http.Features;
using Microsoft.Extensions.DependencyInjection.Extensions;
using Serilog;

var builder = WebApplication.CreateBuilder(args);
var config = builder.Configuration;

// Role: "all" (default: API + worker), "api" or "worker" (conversion worker only; see deploy/docker-compose.yml).
var role = (config["Role"] ?? "all").ToLowerInvariant();
if (role is not ("all" or "api" or "worker"))
{
    throw new InvalidOperationException($"Unknown Role '{role}' (expected all, api or worker).");
}

var serveApi = role != "worker";
if (role != "all" && !string.Equals(config["Jobs:Provider"], "Redis", StringComparison.OrdinalIgnoreCase))
{
    // Split api/worker processes only share jobs through Redis.
    throw new InvalidOperationException($"Role '{role}' requires Jobs:Provider=Redis.");
}

builder.Host.UseSerilog((ctx, cfg) => cfg.ReadFrom.Configuration(ctx.Configuration).Enrich.FromLogContext().WriteTo.Console());
builder.Services.Configure<BoCliOptions>(config.GetSection("BoCli"));
builder.Services.AddSingleton<IDocumentConverter, BoCliConverter>();
builder.Services.AddProblemDetails();
builder.Services.AddExceptionHandler<ApiExceptionHandler>();
builder.Services.AddHttpContextAccessor();
builder.Services.ConfigureHttpJsonOptions(o =>
{
    o.SerializerOptions.Converters.Add(new JsonStringEnumConverter(JsonNamingPolicy.CamelCase));
    o.SerializerOptions.DefaultIgnoreCondition = JsonIgnoreCondition.Never;
});

// Infrastructure (DB, object store, job queue, host clients, webhooks) + auth.
builder.Services.AddBlueOfficeInfrastructure(config);
builder.Services.AddBlueOfficeAuth(config, builder.Environment);
builder.Services.AddBlueOfficeHealth(config);
if (config.GetValue("Otel:Enabled", true))
{
    builder.Services.AddBlueOfficeTelemetry(config);
}

// Application services.
builder.Services.AddScoped<UserService>();
builder.Services.AddScoped<AuditService>();
builder.Services.AddScoped<FileService>();
builder.Services.AddScoped<SessionService>();
builder.Services.AddSingleton<SessionStore>();
builder.Services.AddSingleton<LocalHostStorage>();
builder.Services.Configure<CollabOptions>(config.GetSection("Collab"));
builder.Services.AddSingleton(sp => CollabLogStore.Create(sp, config));
// Several api instances: room leases in Redis (Collab:Ownership=Redis, docs/deploy/README.md).
var instance = config.GetSection("Collab").Get<InstanceOptions>() ?? new InstanceOptions();
builder.Services.AddSingleton(instance);
if (instance.Ownership.Equals("Redis", StringComparison.OrdinalIgnoreCase))
{
    var redisCs = config.GetConnectionString("Redis") ?? "localhost:6379";
    builder.Services.TryAddSingleton<StackExchange.Redis.IConnectionMultiplexer>(_ => StackExchange.Redis.ConnectionMultiplexer.Connect(redisCs));
    builder.Services.TryAddSingleton<BlueOffice.Infrastructure.Jobs.IRedisCommands, BlueOffice.Infrastructure.Jobs.RedisCommands>();
    builder.Services.AddSingleton<IRoomOwnership, RedisRoomOwnership>();
}
else
{
    builder.Services.AddSingleton<IRoomOwnership, LocalRoomOwnership>();
}

builder.Services.AddSingleton<RoomManager>();
builder.Services.AddRevocationProxy(config);
builder.Services.AddSingleton<CollabMaintenanceService>();
if (serveApi)
{
    builder.Services.AddHostedService(sp => sp.GetRequiredService<CollabMaintenanceService>());
}

builder.Services.Configure<WorkerOptions>(config.GetSection("Worker"));
if (role == "worker" || (role == "all" && config.GetValue("Worker:Enabled", true)))
{
    builder.Services.AddHostedService<ConversionWorker>();
}

// Request size limits follow the upload limit.
var maxUpload = config.GetValue("Storage:MaxUploadBytes", new StorageOptions().MaxUploadBytes);
builder.WebHost.ConfigureKestrel(k => k.Limits.MaxRequestBodySize = maxUpload + (1024 * 1024));
builder.Services.Configure<FormOptions>(o => o.MultipartBodyLengthLimit = maxUpload);

// CORS: origins of configured hosts + Cors:Origins.
builder.Services.AddCors(o => o.AddDefaultPolicy(p =>
{
    var origins = config.GetSection("Cors:Origins").Get<string[]>() ?? [];
    var hostOrigins = config.GetSection("Hosts").GetChildren().SelectMany(h => h.GetSection("AllowedOrigins").Get<string[]>() ?? []);
    p.WithOrigins([.. origins.Concat(hostOrigins).Where(o => !string.IsNullOrWhiteSpace(o)).Distinct()]).AllowAnyHeader().AllowAnyMethod().WithExposedHeaders("ETag", "X-Version", "Location");
}));

var app = builder.Build();

// Fail fast: WOPI proof keys must be configured in Production when a WOPI host is configured.
app.Services.GetRequiredService<WopiProofKeys>();

if (config.GetValue("Database:AutoCreate", true))
{
    // EF Core migrations (also adopts databases created by EnsureCreated before 0.7); see docs/deploy/upgrade.md.
    using var scope = app.Services.CreateScope();
    var db = scope.ServiceProvider.GetRequiredService<BlueOfficeDbContext>();
    await DatabaseMigrator.MigrateAsync(db);
}

app.UseExceptionHandler();
app.Use(async (ctx, next) =>
{
    // Baseline security headers; framing is limited to configured host origins.
    var h = ctx.Response.Headers;
    h.XContentTypeOptions = "nosniff";
    h["Referrer-Policy"] = "no-referrer";
    var frameAncestors = string.Join(' ', ctx.RequestServices.GetRequiredService<HostRegistry>().All.SelectMany(x => x.AllowedOrigins).Where(o => !string.IsNullOrWhiteSpace(o)).Distinct());
    h.ContentSecurityPolicy = $"frame-ancestors 'self' {frameAncestors}".TrimEnd();
    await next();
});
app.UseSerilogRequestLogging();
app.UseCors();
app.UseWebSockets(new WebSocketOptions { KeepAliveInterval = TimeSpan.FromSeconds(30) });
app.UseAuthentication();
app.UseAuthorization();

app.MapHealthChecks("/health", new HealthCheckOptions { Predicate = _ => false });
app.MapHealthChecks("/health/ready", new HealthCheckOptions { Predicate = c => c.Tags.Contains(HealthAndTelemetry.ReadyTag) });

app.MapGet("/api/version", () => Results.Ok(new
{
    name = "BlueOffice",
    version = typeof(Program).Assembly.GetName().Version?.ToString(3) ?? "0.0.0",
    phase = 6,
    role,
}));

if (!serveApi)
{
    // Worker role: only health/version endpoints; jobs come from the Redis queue.
    app.Run();
    return;
}

app.MapAuthConfig(config);

// Synchronous conversion through bo-cli (Phase 1 API, kept for compatibility; prefer POST /convert).
app.MapPost("/api/convert/pdf", async (IFormFile file, IDocumentConverter converter, CancellationToken ct) =>
{
    try
    {
        await using var s = file.OpenReadStream();
        var result = await converter.ConvertAsync(s, file.FileName, ConversionTarget.Pdf, ct);
        return Results.File(result.Content, result.ContentType, result.FileName);
    }
    catch (ConversionException e)
    {
        return Results.Problem(e.Message, statusCode: StatusCodes.Status422UnprocessableEntity);
    }
}).DisableAntiforgery().RequireAuthorization(AuthSetup.ServicePolicy);

app.MapFileEndpoints();
app.MapJobEndpoints();
app.MapSessionEndpoints();
app.MapHostBrowseEndpoints();
app.MapRevocationProxy();

app.Run();

/// <summary>Entry point (public for WebApplicationFactory).</summary>
public partial class Program;
