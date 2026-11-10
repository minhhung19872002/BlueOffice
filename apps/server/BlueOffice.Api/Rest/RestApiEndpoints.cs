using System.Text;
using System.Text.Json;
using BlueOffice.Api.Auth;
using BlueOffice.Api.Common;
using BlueOffice.Api.Jobs;
using BlueOffice.Domain;
using BlueOffice.Infrastructure;
using BlueOffice.Infrastructure.Jobs;
using BlueOffice.Infrastructure.Storage;
using Microsoft.AspNetCore.Http.Features;
using Microsoft.Extensions.Options;

namespace BlueOffice.Api.Rest;

/// <summary>
/// Document REST API for other systems (CORE-API-001): convert, bulk mail merge, report generation from a template
/// and inspection, under <c>/api/v1</c>. Each operation is a job of the conversion worker (<c>bo-cli</c> in a temp
/// directory, <c>BoCli:Timeout</c>): the result is returned directly when it is ready within <c>Worker:WaitTimeout</c>,
/// else <c>202</c> with a status URL (<c>?wait=false</c>: always 202). Off unless <c>RestApi:Enabled</c>
/// (docs/deploy/rest-api.md, OpenAPI at <c>/api/v1/openapi.json</c>).
/// </summary>
public static class RestApiEndpoints
{
    /// <summary>Route prefix of this API version.</summary>
    public const string Prefix = "/api/v1";

    private static readonly string[] EditOps = ["setControl", "setBookmark", "replaceText", "setProperties", "setCell"];

    /// <summary>Maps the routes (nothing when the API is disabled).</summary>
    public static void MapRestApi(this WebApplication app)
    {
        var opts = app.Services.GetRequiredService<RestApiOptions>();
        if (!opts.Enabled)
        {
            return;
        }

        app.MapGet(Prefix + "/openapi.json", () => Results.Stream(OpenApiDocument(), "application/json")).AllowAnonymous();
        var group = app.MapGroup(Prefix)
            .RequireAuthorization(ApiKeyAuth.Policy)
            .RequireRateLimiting(ApiKeyAuth.RateLimitPolicy)
            .DisableAntiforgery();
        group.MapPost("/documents/convert", ConvertAsync);
        group.MapPost("/documents/merge", MergeAsync);
        group.MapPost("/documents/edit", EditAsync);
        group.MapPost("/documents/inspect", InspectAsync);
        group.MapGet("/jobs/{id}", JobEndpoints.StatusAsync);
        group.MapGet("/jobs/{id}/result", JobEndpoints.ResultAsync);
    }

    /// <summary>The OpenAPI 3.0 description of this API (embedded <c>Rest/openapi.v1.json</c>).</summary>
    public static Stream OpenApiDocument() =>
        typeof(RestApiEndpoints).Assembly.GetManifestResourceStream("BlueOffice.Api.Rest.openapi.v1.json")
        ?? throw new InvalidOperationException("openapi.v1.json is not embedded.");

    private static async Task<IResult> ConvertAsync(HttpContext ctx, CancellationToken ct)
    {
        ctx.User.RequireScope("convert");
        var form = await ReadFormAsync(ctx, ct);
        var file = RequiredFile(ctx, form, "file");
        var ext = InputExtension(ctx, file);
        var to = Field(form, "to", "pdf").ToLowerInvariant();
        if (!ConversionFormats.TargetsFor(ext).Contains(to))
        {
            throw ApiException.BadRequest($"Cannot convert '{ext}' to '{to}' (supported: {string.Join(", ", ConversionFormats.TargetsFor(ext))}).");
        }

        var job = NewJob(ctx, JobKind.Convert, file, to);
        JobEndpoints.ReadPdfOptions(form, job);
        if (job.Options.Count > 0 && to != "pdf")
        {
            throw ApiException.BadRequest("'pdfa', 'tagged' and 'bookmarks' apply to PDF output only.");
        }

        await StoreAsync(ctx, job.InputKey, file, ct);
        return await SubmitAsync(ctx, job, ct);
    }

    private static async Task<IResult> MergeAsync(HttpContext ctx, CancellationToken ct)
    {
        ctx.User.RequireScope("merge");
        var opts = ctx.RequestServices.GetRequiredService<RestApiOptions>();
        var form = await ReadFormAsync(ctx, ct);
        var template = RequiredFile(ctx, form, "template");
        if (!ConversionFormats.IsMergeTemplate(InputExtension(ctx, template)))
        {
            throw ApiException.BadRequest("'template' must be a .docx document.");
        }

        var to = Field(form, "to", "docx").ToLowerInvariant();
        if (to is not ("docx" or "pdf"))
        {
            throw ApiException.BadRequest("'to' must be 'docx' or 'pdf'.");
        }

        var output = Field(form, "output", "single").ToLowerInvariant();
        if (output is not ("single" or "zip"))
        {
            throw ApiException.BadRequest("'output' must be 'single' (one merged document) or 'zip' (one file per record).");
        }

        var pattern = Field(form, "pattern", "");
        if (pattern.Length > 200)
        {
            throw ApiException.BadRequest("'pattern' is too long (200 characters at most).");
        }

        var job = NewJob(ctx, JobKind.Merge, template, output == "zip" ? "zip" : to);
        job.Options["each"] = to;
        if (pattern.Length > 0)
        {
            job.Options["pattern"] = pattern;
        }

        var store = ctx.RequestServices.GetRequiredService<IObjectStore>();
        var data = form.Files.GetFile("data");
        var records = (string?)form["records"];
        if ((data is null) == string.IsNullOrWhiteSpace(records))
        {
            throw ApiException.BadRequest("Send the merge data either as the file 'data' (.csv, .json, .xlsx, .vcf) or as the JSON array 'records'.");
        }

        if (data is not null)
        {
            CheckSize(ctx, data);
            job.DataExtension = Path.GetExtension(data.FileName).ToLowerInvariant();
            if (!ConversionFormats.MergeData.Contains(job.DataExtension))
            {
                throw ApiException.BadRequest($"Unsupported merge data type '{job.DataExtension}' (supported: {string.Join(", ", ConversionFormats.MergeData)}).");
            }

            await StoreAsync(ctx, job.DataKey, data, ct);
        }
        else
        {
            var count = JsonArrayLength(records!, "records", JsonValueKind.Object, null);
            if (count == 0)
            {
                throw ApiException.BadRequest("'records' is empty.");
            }

            if (count > opts.MaxRecords)
            {
                throw ApiException.BadRequest($"Too many records ({count}; at most {opts.MaxRecords}).");
            }

            job.DataExtension = ".json";
            using var ms = new MemoryStream(Encoding.UTF8.GetBytes(records!));
            await store.PutAsync(job.DataKey, ms, "application/json", ct);
        }

        await StoreAsync(ctx, job.InputKey, template, ct);
        return await SubmitAsync(ctx, job, ct);
    }

    private static async Task<IResult> EditAsync(HttpContext ctx, CancellationToken ct)
    {
        ctx.User.RequireScope("edit");
        var opts = ctx.RequestServices.GetRequiredService<RestApiOptions>();
        var form = await ReadFormAsync(ctx, ct);
        var file = RequiredFile(ctx, form, "file");
        var own = ConversionFormats.EditFormat(InputExtension(ctx, file))
            ?? throw ApiException.BadRequest("Edits apply to .docx and .xlsx templates.");
        var to = Field(form, "to", own).ToLowerInvariant();
        if (to != own && to != "pdf")
        {
            throw ApiException.BadRequest($"'to' must be '{own}' or 'pdf'.");
        }

        // Large edit lists may come as a file part named "edits" (form values are limited to 4 MB).
        var edits = (string?)form["edits"];
        if (string.IsNullOrWhiteSpace(edits) && form.Files.GetFile("edits") is { } part)
        {
            CheckSize(ctx, part);
            using var reader = new StreamReader(part.OpenReadStream(), Encoding.UTF8);
            edits = await reader.ReadToEndAsync(ct);
        }

        if (string.IsNullOrWhiteSpace(edits))
        {
            throw ApiException.BadRequest("Missing 'edits' (JSON array).");
        }

        var count = JsonArrayLength(edits, "edits", JsonValueKind.Object, EditOps);
        if (count == 0)
        {
            throw ApiException.BadRequest("'edits' is empty.");
        }

        if (count > opts.MaxEdits)
        {
            throw ApiException.BadRequest($"Too many edits ({count}; at most {opts.MaxEdits}).");
        }

        var job = NewJob(ctx, JobKind.Edit, file, to);
        if (form["strict"].Count == 0 || JobEndpoints.IsTrue(form["strict"]))
        {
            // Default: an edit that matches nothing fails the request (a report must not go out half filled).
            job.Options["strict"] = "true";
        }

        var store = ctx.RequestServices.GetRequiredService<IObjectStore>();
        using (var ms = new MemoryStream(Encoding.UTF8.GetBytes(edits)))
        {
            await store.PutAsync(job.DataKey, ms, "application/json", ct);
        }

        await StoreAsync(ctx, job.InputKey, file, ct);
        return await SubmitAsync(ctx, job, ct);
    }

    private static async Task<IResult> InspectAsync(HttpContext ctx, CancellationToken ct)
    {
        ctx.User.RequireScope("inspect");
        var form = await ReadFormAsync(ctx, ct);
        var file = RequiredFile(ctx, form, "file");
        var job = NewJob(ctx, JobKind.Inspect, file, "json");
        await StoreAsync(ctx, job.InputKey, file, ct);
        return await SubmitAsync(ctx, job, ct);
    }

    private static Task<IResult> SubmitAsync(HttpContext ctx, ConversionJob job, CancellationToken ct)
    {
        ctx.RequestServices.GetRequiredService<ILoggerFactory>().CreateLogger("BlueOffice.RestApi")
            .LogInformation("REST {Kind} job {Job} by {Principal}", job.Kind, job.Id, job.Owner);

        // Synchronous by default (with the worker's wait timeout as the hard limit), ?wait=false: 202 at once.
        var wait = !string.Equals(ctx.Request.Query["wait"], "false", StringComparison.OrdinalIgnoreCase);
        return JobEndpoints.EnqueueAsync(ctx, job, wait, ct);
    }

    private static ConversionJob NewJob(HttpContext ctx, JobKind kind, IFormFile file, string target) => new()
    {
        Kind = kind,
        Status = JobStatus.Queued,
        FileName = Path.GetFileName(file.FileName),
        TargetFormat = target,
        Owner = ctx.User.PrincipalKey(),
        UrlPrefix = Prefix,
        CreatedAt = ctx.RequestServices.GetRequiredService<TimeProvider>().GetUtcNow().UtcDateTime,
    };

    private static long Limit(HttpContext ctx) => Math.Min(
        ctx.RequestServices.GetRequiredService<RestApiOptions>().MaxUploadBytes,
        ctx.RequestServices.GetRequiredService<StorageOptions>().MaxUploadBytes);

    private static async Task<IFormCollection> ReadFormAsync(HttpContext ctx, CancellationToken ct)
    {
        if (!ctx.Request.HasFormContentType)
        {
            throw ApiException.BadRequest("Expected multipart/form-data.");
        }

        // Two files at most (template + data) plus small fields: refuse larger bodies before reading them.
        var max = (2 * Limit(ctx)) + (1024 * 1024);
        if (ctx.Request.ContentLength > max)
        {
            throw new PayloadTooLargeException(Limit(ctx));
        }

        if (ctx.Features.Get<IHttpMaxRequestBodySizeFeature>() is { IsReadOnly: false } body && (body.MaxRequestBodySize is null || body.MaxRequestBodySize > max))
        {
            body.MaxRequestBodySize = max;
        }

        try
        {
            return await ctx.Request.ReadFormAsync(ct);
        }
        catch (InvalidDataException e)
        {
            throw ApiException.BadRequest("Malformed multipart body: " + e.Message);
        }
    }

    private static void CheckSize(HttpContext ctx, IFormFile file)
    {
        if (file.Length > Limit(ctx))
        {
            throw new PayloadTooLargeException(Limit(ctx));
        }
    }

    private static IFormFile RequiredFile(HttpContext ctx, IFormCollection form, string name)
    {
        var file = form.Files.GetFile(name) ?? throw ApiException.BadRequest($"Missing file '{name}'.");
        if (file.Length == 0)
        {
            throw ApiException.BadRequest($"File '{name}' is empty.");
        }

        CheckSize(ctx, file);
        return file;
    }

    /// <summary>Extension of an input the worker accepts (<c>BoCli:AllowedExtensions</c>), else 400.</summary>
    private static string InputExtension(HttpContext ctx, IFormFile file)
    {
        var ext = Path.GetExtension(file.FileName).ToLowerInvariant();
        var allowed = ctx.RequestServices.GetRequiredService<IOptions<BoCliOptions>>().Value.AllowedExtensions;
        return allowed.Contains(ext, StringComparer.OrdinalIgnoreCase)
            ? ext
            : throw ApiException.BadRequest($"Unsupported input type '{ext}'.");
    }

    private static string Field(IFormCollection form, string name, string fallback) =>
        (string?)form[name] is { Length: > 0 } v ? v.Trim() : fallback;

    private static async Task StoreAsync(HttpContext ctx, string key, IFormFile file, CancellationToken ct)
    {
        await using var s = file.OpenReadStream();
        await ctx.RequestServices.GetRequiredService<IObjectStore>().PutAsync(key, s, "application/octet-stream", ct);
    }

    /// <summary>
    /// Length of the JSON array <paramref name="json"/> whose items are all of <paramref name="kind"/> (and, with
    /// <paramref name="ops"/>, carry a known <c>op</c>); 400 otherwise. The engine validates the rest.
    /// </summary>
    private static int JsonArrayLength(string json, string name, JsonValueKind kind, string[]? ops)
    {
        try
        {
            using var doc = JsonDocument.Parse(json, new JsonDocumentOptions { MaxDepth = 16 });
            if (doc.RootElement.ValueKind != JsonValueKind.Array)
            {
                throw ApiException.BadRequest($"'{name}' must be a JSON array.");
            }

            var n = 0;
            foreach (var item in doc.RootElement.EnumerateArray())
            {
                if (item.ValueKind != kind)
                {
                    throw ApiException.BadRequest($"'{name}[{n}]' must be a JSON object.");
                }

                if (ops is not null && !(item.TryGetProperty("op", out var op) && op.ValueKind == JsonValueKind.String && ops.Contains(op.GetString())))
                {
                    throw ApiException.BadRequest($"'{name}[{n}].op' must be one of: {string.Join(", ", ops)}.");
                }

                n++;
            }

            return n;
        }
        catch (JsonException e)
        {
            throw ApiException.BadRequest($"'{name}' is not valid JSON: {e.Message}");
        }
    }
}
