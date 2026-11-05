using System.Collections.Concurrent;
using System.Security.Cryptography;
using System.Text;
using BlueOffice.Domain;

namespace BlueOffice.Api.Sessions;

/// <summary>An editor session: one user editing/viewing one file of one host. Identified by a public id + secret token.</summary>
public sealed class EditorSession
{
    /// <summary>Public id (appears in URLs).</summary>
    public required string Id { get; init; }

    /// <summary>Secret token (header <c>X-Session-Token</c>, WebSocket <c>?token=</c>).</summary>
    public required string Token { get; init; }

    /// <summary>Host.</summary>
    public required HostConfig Host { get; init; }

    /// <summary>Storage adapter of the host.</summary>
    public required IHostStorage Storage { get; init; }

    /// <summary>File id in the host.</summary>
    public required string FileId { get; init; }

    /// <summary>Collaboration room key ("&lt;host&gt;/&lt;file id&gt;").</summary>
    public string RoomKey => $"{Host.Name}/{FileId}";

    /// <summary>Access token used to call the host on behalf of the user (local: internal user id).</summary>
    public required string AccessToken { get; set; }

    /// <summary>Last known file metadata.</summary>
    public required HostFileInfo File { get; set; }

    /// <summary>User.</summary>
    public HostUser User => File.User;

    /// <summary>Current permissions.</summary>
    public HostPermissions Permissions => File.Permissions;

    /// <summary>Creation time.</summary>
    public DateTime CreatedAt { get; init; }

    /// <summary>Expiry (sliding, extended by refresh).</summary>
    public DateTime ExpiresAt { get; set; }

    /// <summary>Host call context.</summary>
    public HostCallContext Context => new(Host, AccessToken);
}

/// <summary>In-memory session registry (single API instance per document; see ADR-0010).</summary>
public sealed class SessionStore
{
    private readonly ConcurrentDictionary<string, EditorSession> _sessions = new();

    /// <summary>Adds a session.</summary>
    public void Add(EditorSession s) => _sessions[s.Id] = s;

    /// <summary>Session by id + secret (constant-time comparison); null if unknown, wrong token or expired.</summary>
    public EditorSession? Get(string? id, string? token, DateTime nowUtc)
    {
        if (id is null || token is null || !_sessions.TryGetValue(id, out var s) || s.ExpiresAt <= nowUtc)
        {
            return null;
        }

        return CryptographicOperations.FixedTimeEquals(Encoding.UTF8.GetBytes(s.Token), Encoding.UTF8.GetBytes(token)) ? s : null;
    }

    /// <summary>Removes a session.</summary>
    public bool Remove(string id) => _sessions.TryRemove(id, out _);

    /// <summary>All sessions.</summary>
    public IReadOnlyList<EditorSession> All => _sessions.Values.ToList();

    /// <summary>Removes expired sessions; returns them.</summary>
    public IReadOnlyList<EditorSession> PurgeExpired(DateTime nowUtc)
    {
        var expired = _sessions.Values.Where(s => s.ExpiresAt <= nowUtc).ToList();
        foreach (var s in expired)
        {
            _sessions.TryRemove(s.Id, out _);
        }

        return expired;
    }
}
