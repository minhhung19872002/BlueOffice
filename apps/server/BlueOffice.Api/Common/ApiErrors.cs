using BlueOffice.Domain;
using BlueOffice.Infrastructure.Hosts;
using Microsoft.AspNetCore.Diagnostics;
using Microsoft.AspNetCore.Mvc;

namespace BlueOffice.Api.Common;

/// <summary>Error with an HTTP status, turned into RFC 7807 problem details.</summary>
public sealed class ApiException(int status, string message) : Exception(message)
{
    /// <summary>HTTP status.</summary>
    public int Status { get; } = status;

    /// <summary>404 (also used instead of 403 when the caller may not know the resource exists).</summary>
    public static ApiException NotFound(string what = "Resource") => new(StatusCodes.Status404NotFound, $"{what} not found.");

    /// <summary>403.</summary>
    public static ApiException Forbidden(string message = "Forbidden.") => new(StatusCodes.Status403Forbidden, message);

    /// <summary>400.</summary>
    public static ApiException BadRequest(string message) => new(StatusCodes.Status400BadRequest, message);

    /// <summary>409.</summary>
    public static ApiException Conflict(string message) => new(StatusCodes.Status409Conflict, message);
}

/// <summary>Maps domain/host exceptions to problem details.</summary>
public sealed class ApiExceptionHandler(IProblemDetailsService problems, ILogger<ApiExceptionHandler> logger) : IExceptionHandler
{
    /// <inheritdoc />
    public async ValueTask<bool> TryHandleAsync(HttpContext httpContext, Exception exception, CancellationToken cancellationToken)
    {
        var (status, detail) = exception switch
        {
            ApiException e => (e.Status, e.Message),
            PayloadTooLargeException e => (StatusCodes.Status413PayloadTooLarge, e.Message),
            HostTokenException e => (StatusCodes.Status401Unauthorized, e.Message),
            HostConflictException e => (StatusCodes.Status409Conflict, e.Message),
            HostNotSupportedException e => (StatusCodes.Status501NotImplemented, e.Message),
            HostException { StatusCode: 401 or 403 } => (StatusCodes.Status403Forbidden, "The host denied access."),
            HostException { StatusCode: 404 } => (StatusCodes.Status404NotFound, "File not found on host."),
            HostException { StatusCode: 400 or 410 or 413 } e => (e.StatusCode, e.Message),
            HostException e => (StatusCodes.Status502BadGateway, e.Message),
            BadHttpRequestException e => (e.StatusCode, e.Message),
            _ => (0, ""),
        };
        if (status == 0)
        {
            return false;
        }

        if (status >= 500)
        {
            logger.LogWarning(exception, "Request failed with {Status}", status);
        }

        httpContext.Response.StatusCode = status;
        return await problems.TryWriteAsync(new ProblemDetailsContext
        {
            HttpContext = httpContext,
            ProblemDetails = new ProblemDetails { Status = status, Detail = detail },
            Exception = exception,
        });
    }
}
