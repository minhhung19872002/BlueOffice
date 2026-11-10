namespace BlueOffice.Domain;

/// <summary>
/// An add-in manifest published to the organisation catalogue by an administrator (CORE-ADDIN-001, ADR-0013).
/// The manifest JSON is stored as published (validated by <c>AddinManifestValidator</c>); the editors validate it
/// again before loading the add-in.
/// </summary>
public sealed class AddinEntry
{
    /// <summary>Manifest id (reverse domain name, e.g. <c>vn.example.hello</c>); primary key.</summary>
    public string Id { get; set; } = "";

    /// <summary>Manifest version (semantic version).</summary>
    public string Version { get; set; } = "";

    /// <summary>Vietnamese name (listing, audit).</summary>
    public string Name { get; set; } = "";

    /// <summary>Applications the add-in supports, space separated (<c>doc sheet slide</c>).</summary>
    public string Apps { get; set; } = "";

    /// <summary>The manifest JSON.</summary>
    public string Manifest { get; set; } = "";

    /// <summary>Set by the administrator: the add-in frame may keep its own origin (<c>allow-same-origin</c>).</summary>
    public bool Trusted { get; set; }

    /// <summary>False hides the add-in from users without deleting it (installs are kept).</summary>
    public bool Enabled { get; set; } = true;

    /// <summary>Principal key of the administrator who published (or last updated) it.</summary>
    public string PublishedBy { get; set; } = "";

    /// <summary>First publication (UTC).</summary>
    public DateTime CreatedAt { get; set; }

    /// <summary>Last update (UTC).</summary>
    public DateTime UpdatedAt { get; set; }
}

/// <summary>One user's install of a catalogue add-in, with the permissions the user consented to.</summary>
public sealed class AddinInstall
{
    /// <summary>Id (32 hex chars).</summary>
    public string Id { get; set; } = Guid.NewGuid().ToString("N");

    /// <summary><see cref="AddinEntry.Id"/>.</summary>
    public string AddinId { get; set; } = "";

    /// <summary>Who installed it: <c>user:&lt;sub&gt;</c> (signed-in) or <c>host:&lt;host&gt;:&lt;user id&gt;</c> (editor session).</summary>
    public string UserKey { get; set; } = "";

    /// <summary>Granted permissions, space separated.</summary>
    public string Permissions { get; set; } = "";

    /// <summary>Manifest version consented to.</summary>
    public string Version { get; set; } = "";

    /// <summary>Install time (UTC).</summary>
    public DateTime InstalledAt { get; set; }
}
