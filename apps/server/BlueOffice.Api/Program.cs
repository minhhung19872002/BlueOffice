using System.Text.Json;
using System.Text.Json.Serialization;
using BlueOffice.Api.Audit;
using BlueOffice.Api.Auth;
using BlueOffice.Api.Collab;
using BlueOffice.Api.Common;
using BlueOffice.Api.Files;
using BlueOffice.Api.Jobs;
using BlueOffice.Api.Sessions;
using BlueOffice.Domain;
using BlueOffice.Infrastructure;
using BlueOffice.Infrastructure.Data;
using BlueOffice.Infrastructure.Hosts;
using BlueOffice.Infrastructure.Storage;
using Microsoft.AspNetCore.Diagnostics.HealthChecks;
using Microsoft.AspNetCore.Http.Features;
using Serilog;

var builder = WebApplication.CreateBuilder(args);
var config = builder.Configuration;

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
builder.Services.AddSingleton<RoomManager>();
builder.Services.AddSingleton<CollabMaintenanceService>();
builder.Services.AddHostedService(sp => sp.GetRequiredService<CollabMaintenanceService>());
builder.Services.Configure<WorkerOptions>(config.GetSection("Worker"));
if (config.GetValue("Worker:Enabled", true))
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
    p.WithOrigins([.. origins.Concat(hostOrigins).Distinct()]).AllowAnyHeader().AllowAnyMethod().WithExposedHeaders("ETag", "X-Version", "Location");
}));

var app = builder.Build();

if (config.GetValue("Database:AutoCreate", true))
{
    // Schema creation for dev/test and first start; see docs/deploy.md for production upgrades.
    using var scope = app.Services.CreateScope();
    scope.ServiceProvider.GetRequiredService<BlueOfficeDbContext>().Database.EnsureCreated();
}

app.UseExceptionHandler();
app.Use(async (ctx, next) =>
{
    // Baseline security headers; framing is limited to configured host origins.
    var h = ctx.Response.Headers;
    h.XContentTypeOptions = "nosniff";
    h["Referrer-Policy"] = "no-referrer";
    var frameAncestors = string.Join(' ', ctx.RequestServices.GetRequiredService<HostRegistry>().All.SelectMany(x => x.AllowedOrigins).Distinct());
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
}));

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

app.Run();

/// <summary>Entry point (public for WebApplicationFactory).</summary>
public partial class Program;
