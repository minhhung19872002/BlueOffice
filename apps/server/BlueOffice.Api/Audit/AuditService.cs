using BlueOffice.Domain;
using BlueOffice.Infrastructure.Data;
using BlueOffice.Infrastructure.Hosts;

namespace BlueOffice.Api.Audit;

/// <summary>Writes audit entries and forwards host-related ones to the host as <c>audit</c> webhooks.</summary>
public sealed class AuditService(
    BlueOfficeDbContext db,
    IHostNotifier notifier,
    IHttpContextAccessor http,
    TimeProvider time)
{
    /// <summary>Records an action. <paramref name="forwardTo"/> = host that should receive the event.</summary>
    public async Task RecordAsync(
        AuditAction action,
        ResourceKind kind,
        string resourceId,
        string actor,
        string? actorName,
        string? detail,
        HostConfig? forwardTo,
        CancellationToken ct)
    {
        var entry = new AuditEntry
        {
            At = time.GetUtcNow().UtcDateTime,
            Action = action,
            ResourceKind = kind,
            ResourceId = resourceId,
            Actor = actor,
            ActorName = actorName,
            Detail = detail,
            Host = forwardTo?.Name ?? "local",
            IpAddress = http.HttpContext?.Connection.RemoteIpAddress?.ToString(),
        };
        db.AuditEntries.Add(entry);
        await db.SaveChangesAsync(ct);
        if (forwardTo is not null && forwardTo.Protocol != HostProtocol.Local)
        {
            notifier.Publish(forwardTo, HostEvents.Audit, resourceId, new
            {
                action = action.ToString().ToLowerInvariant(),
                actor,
                actorName,
                at = entry.At,
                detail,
                ip = entry.IpAddress,
            });
        }
    }
}
