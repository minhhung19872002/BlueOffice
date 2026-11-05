using BlueOffice.Domain;
using BlueOffice.Infrastructure;
using Serilog;

var builder = WebApplication.CreateBuilder(args);

builder.Host.UseSerilog((ctx, cfg) => cfg.ReadFrom.Configuration(ctx.Configuration).WriteTo.Console());
builder.Services.Configure<BoCliOptions>(builder.Configuration.GetSection("BoCli"));
builder.Services.AddSingleton<IDocumentConverter, BoCliConverter>();
builder.Services.AddProblemDetails();

var app = builder.Build();

app.UseExceptionHandler();

app.MapGet("/health", () => Results.Ok(new { status = "ok" }));

app.MapGet("/api/version", () => Results.Ok(new
{
    name = "BlueOffice",
    version = typeof(Program).Assembly.GetName().Version?.ToString(3) ?? "0.0.0",
    phase = 1,
}));

// Headless conversion through the Rust core (bo-cli). Full worker queue arrives in Phase 6.
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
}).DisableAntiforgery();

app.Run();

/// <summary>Entry point (public for WebApplicationFactory).</summary>
public partial class Program;
