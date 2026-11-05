using System.Security.Claims;
using BlueOffice.Api.Auth;
using BlueOffice.Api.Common;
using BlueOffice.Domain;
using Microsoft.Net.Http.Headers;

namespace BlueOffice.Api.Files;

/// <summary>REST API for folders, files, versions, shares, links and audit (Phase 6).</summary>
public static class FileEndpoints
{
    /// <summary>Maps the routes under /api.</summary>
    public static void MapFileEndpoints(this IEndpointRouteBuilder app)
    {
        var api = app.MapGroup("/api").RequireAuthorization(AuthSetup.UserPolicy);

        api.MapGet("/me", async (ClaimsPrincipal p, UserService users, CancellationToken ct) =>
        {
            var u = await users.CurrentAsync(p, ct);
            return Results.Ok(new { u.Id, u.Subject, u.DisplayName, u.Email });
        });

        // Folders
        api.MapGet("/folders", async (ClaimsPrincipal p, UserService users, FileService files, CancellationToken ct) =>
            Results.Ok(await files.ListRootAsync((await users.CurrentAsync(p, ct)).Id, ct)));
        api.MapGet("/shared", async (ClaimsPrincipal p, UserService users, FileService files, CancellationToken ct) =>
            Results.Ok(await files.ListSharedWithMeAsync((await users.CurrentAsync(p, ct)).Id, ct)));
        api.MapPost("/folders", async (CreateFolderRequest req, ClaimsPrincipal p, UserService users, FileService files, CancellationToken ct) =>
        {
            var f = await files.CreateFolderAsync(await users.CurrentAsync(p, ct), req, ct);
            return Results.Created($"/api/folders/{f.Id}", f);
        });
        api.MapGet("/folders/{id:guid}", async (Guid id, ClaimsPrincipal p, UserService users, FileService files, CancellationToken ct) =>
            Results.Ok(await files.ListFolderAsync((await users.CurrentAsync(p, ct)).Id, id, ct)));
        api.MapPatch("/folders/{id:guid}", async (Guid id, UpdateRequest req, ClaimsPrincipal p, UserService users, FileService files, CancellationToken ct) =>
            Results.Ok(await files.UpdateFolderAsync((await users.CurrentAsync(p, ct)).Id, id, req, ct)));
        api.MapDelete("/folders/{id:guid}", async (Guid id, ClaimsPrincipal p, UserService users, FileService files, CancellationToken ct) =>
        {
            await files.DeleteFolderAsync(await users.CurrentAsync(p, ct), id, ct);
            return Results.NoContent();
        });

        // Files: upload is a raw streamed body (?name=&folderId=), never buffered in memory.
        api.MapPost("/files", async (HttpRequest request, string name, Guid? folderId, ClaimsPrincipal p, UserService users, FileService files, CancellationToken ct) =>
        {
            var f = await files.UploadAsync(await users.CurrentAsync(p, ct), folderId, name, request.Body, ct);
            return Results.Created($"/api/files/{f.Id}", f);
        });
        api.MapGet("/files/{id:guid}", async (Guid id, ClaimsPrincipal p, UserService users, FileService files, CancellationToken ct) =>
        {
            var (f, role) = await files.GetFileAsync((await users.CurrentAsync(p, ct)).Id, id, Operation.Read, ct);
            return Results.Ok(FileDto.From(f, role));
        });
        api.MapGet("/files/{id:guid}/content", async (Guid id, int? version, ClaimsPrincipal p, UserService users, FileService files, CancellationToken ct) =>
        {
            var u = await users.CurrentAsync(p, ct);
            var (f, v, s) = await files.OpenContentAsync(u.Id, u.DisplayName, id, version, auditDownload: true, ct);
            return Results.Stream(s, f.ContentType, f.Name, entityTag: new EntityTagHeaderValue($"\"{v.Number}\""));
        });
        api.MapPut("/files/{id:guid}/content", async (Guid id, HttpRequest request, ClaimsPrincipal p, UserService users, FileService files, CancellationToken ct) =>
        {
            var u = await users.CurrentAsync(p, ct);
            int? ifMatch = null;
            var etag = request.Headers.IfMatch.ToString().Trim('"', ' ');
            if (etag.Length > 0)
            {
                ifMatch = int.TryParse(etag, out var v) ? v : throw ApiException.BadRequest("If-Match must be a version number.");
            }

            return Results.Ok(await files.PutContentAsync(u.Id, u.DisplayName, id, request.Body, ifMatch, null, ct));
        });
        api.MapPatch("/files/{id:guid}", async (Guid id, UpdateRequest req, ClaimsPrincipal p, UserService users, FileService files, CancellationToken ct) =>
            Results.Ok(await files.UpdateFileAsync(await users.CurrentAsync(p, ct), id, req, ct)));
        api.MapDelete("/files/{id:guid}", async (Guid id, ClaimsPrincipal p, UserService users, FileService files, CancellationToken ct) =>
        {
            await files.DeleteFileAsync(await users.CurrentAsync(p, ct), id, ct);
            return Results.NoContent();
        });

        // Versions
        api.MapGet("/files/{id:guid}/versions", async (Guid id, ClaimsPrincipal p, UserService users, FileService files, CancellationToken ct) =>
            Results.Ok(await files.ListVersionsAsync((await users.CurrentAsync(p, ct)).Id, id, ct)));
        api.MapPost("/files/{id:guid}/versions/{n:int}/restore", async (Guid id, int n, ClaimsPrincipal p, UserService users, FileService files, CancellationToken ct) =>
            Results.Ok(await files.RestoreAsync(await users.CurrentAsync(p, ct), id, n, ct)));

        // Shares
        MapShares(api, "/files/{id:guid}/shares", ResourceKind.File);
        MapShares(api, "/folders/{id:guid}/shares", ResourceKind.Folder);
        api.MapDelete("/shares/{shareId:guid}", async (Guid shareId, ClaimsPrincipal p, UserService users, FileService files, CancellationToken ct) =>
        {
            await files.DeleteShareAsync(await users.CurrentAsync(p, ct), shareId, ct);
            return Results.NoContent();
        });
        api.MapPost("/files/{id:guid}/links", async (Guid id, CreateLinkRequest req, ClaimsPrincipal p, UserService users, FileService files, CancellationToken ct) =>
        {
            var s = await files.CreateLinkAsync(await users.CurrentAsync(p, ct), id, req, ct);
            return Results.Created($"/api/links/{s.LinkToken}", s);
        });

        // Audit
        api.MapGet("/files/{id:guid}/audit", async (Guid id, ClaimsPrincipal p, UserService users, FileService files, CancellationToken ct) =>
            Results.Ok(await files.GetAuditAsync((await users.CurrentAsync(p, ct)).Id, id, ct)));
        api.MapPost("/files/{id:guid}/audit", async (Guid id, ClientAuditRequest req, ClaimsPrincipal p, UserService users, FileService files, CancellationToken ct) =>
        {
            await files.RecordClientActionAsync(await users.CurrentAsync(p, ct), id, req, ct);
            return Results.NoContent();
        });

        // Anonymous link access (the token is the secret).
        var links = app.MapGroup("/api/links").AllowAnonymous();
        links.MapGet("/{token}", async (string token, FileService files, CancellationToken ct) =>
        {
            var (share, f) = await files.ResolveLinkAsync(token, ct);
            return Results.Ok(new { file = FileDto.From(f, share.Role), share.ExpiresAt });
        });
        links.MapGet("/{token}/content", async (string token, FileService files, CancellationToken ct) =>
        {
            var (f, s) = await files.OpenLinkContentAsync(token, ct);
            return Results.Stream(s, f.ContentType, f.Name);
        });
    }

    private static void MapShares(RouteGroupBuilder api, string route, ResourceKind kind)
    {
        api.MapGet(route, async (Guid id, ClaimsPrincipal p, UserService users, FileService files, CancellationToken ct) =>
            Results.Ok(await files.ListSharesAsync((await users.CurrentAsync(p, ct)).Id, kind, id, ct)));
        api.MapPost(route, async (Guid id, CreateShareRequest req, ClaimsPrincipal p, UserService users, FileService files, CancellationToken ct) =>
        {
            var owner = await users.CurrentAsync(p, ct);
            var grantee = await users.FindAsync(req.User, ct) ?? throw ApiException.NotFound("User");
            return Results.Ok(await files.ShareAsync(owner, kind, id, grantee, req.Role, ct));
        });
    }
}
