namespace BlueOffice.Domain;

/// <summary>Operations checked by <see cref="AccessEvaluator"/>.</summary>
public enum Operation
{
    /// <summary>Read metadata / list.</summary>
    Read,

    /// <summary>Download raw content.</summary>
    Download,

    /// <summary>Add a comment.</summary>
    Comment,

    /// <summary>Upload a new version, restore a version, rename, create inside a folder.</summary>
    Edit,

    /// <summary>Delete.</summary>
    Delete,

    /// <summary>Create/remove shares and links.</summary>
    ManageShares,

    /// <summary>Read the audit log.</summary>
    ReadAudit,
}

/// <summary>Pure permission rules (no I/O): effective role and role → operation matrix.</summary>
public static class AccessEvaluator
{
    /// <summary>Minimum role required for an operation.</summary>
    public static ShareRole RequiredRole(Operation op) => op switch
    {
        Operation.Read or Operation.Download => ShareRole.Viewer,
        Operation.Comment => ShareRole.Commenter,
        Operation.Edit => ShareRole.Editor,
        Operation.Delete or Operation.ManageShares or Operation.ReadAudit => ShareRole.Owner,
        _ => ShareRole.Owner,
    };

    /// <summary>True if <paramref name="role"/> allows <paramref name="op"/>.</summary>
    public static bool Allows(ShareRole role, Operation op) => role >= RequiredRole(op);

    /// <summary>
    /// Effective role of <paramref name="userId"/> on a resource.
    /// <paramref name="owners"/>: owner of the resource and of each ancestor folder.
    /// <paramref name="shares"/>: user shares on the resource and on each ancestor folder.
    /// Expired shares are ignored. Owning an ancestor folder makes the user owner of its content.
    /// </summary>
    public static ShareRole EffectiveRole(Guid userId, IEnumerable<Guid> owners, IEnumerable<Share> shares, DateTime nowUtc)
    {
        if (owners.Contains(userId))
        {
            return ShareRole.Owner;
        }

        var best = ShareRole.None;
        foreach (var s in shares)
        {
            if (s.GranteeUserId == userId && IsActive(s, nowUtc) && s.Role > best)
            {
                best = s.Role;
            }
        }

        return best;
    }

    /// <summary>True if the share has not expired.</summary>
    public static bool IsActive(Share share, DateTime nowUtc) => share.ExpiresAt is null || share.ExpiresAt > nowUtc;
}
