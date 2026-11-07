using System.Security.Claims;
using BlueOffice.Api.Common;
using BlueOffice.Domain;
using BlueOffice.Infrastructure.Data;
using Microsoft.EntityFrameworkCore;

namespace BlueOffice.Api.Auth;

/// <summary>Maps the authenticated OIDC principal to a <see cref="User"/> row (created on first use).</summary>
public sealed class UserService(BlueOfficeDbContext db, TimeProvider time)
{
    /// <summary>Current user; throws 401 if the principal has no subject.</summary>
    public async Task<User> CurrentAsync(ClaimsPrincipal principal, CancellationToken ct)
    {
        var sub = principal.FindFirst("sub")?.Value
            ?? throw new ApiException(StatusCodes.Status401Unauthorized, "Token has no subject.");
        var user = await db.Users.FirstOrDefaultAsync(u => u.Subject == sub, ct);
        var name = principal.FindFirst("name")?.Value ?? principal.FindFirst("preferred_username")?.Value ?? sub;
        var email = principal.FindFirst("email")?.Value;
        if (user is null)
        {
            user = new User { Subject = sub, DisplayName = name, Email = email, CreatedAt = time.GetUtcNow().UtcDateTime };
            db.Users.Add(user);
            try
            {
                await db.SaveChangesAsync(ct);
            }
            catch (DbUpdateException)
            {
                // Concurrent first request: another request created it.
                db.Entry(user).State = EntityState.Detached;
                user = await db.Users.FirstAsync(u => u.Subject == sub, ct);
            }
        }
        else if (user.DisplayName != name || user.Email != email)
        {
            user.DisplayName = name;
            user.Email = email;
            await db.SaveChangesAsync(ct);
        }

        return user;
    }

    /// <summary>Finds a user by id, subject or e-mail.</summary>
    public Task<User?> FindAsync(string idOrSubjectOrEmail, CancellationToken ct) =>
        Guid.TryParse(idOrSubjectOrEmail, out var id)
            ? db.Users.FirstOrDefaultAsync(u => u.Id == id, ct)
            : db.Users.FirstOrDefaultAsync(u => u.Subject == idOrSubjectOrEmail || u.Email == idOrSubjectOrEmail, ct);
}
