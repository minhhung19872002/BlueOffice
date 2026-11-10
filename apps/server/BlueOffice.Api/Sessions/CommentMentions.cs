using System.Globalization;
using System.Text;
using BlueOffice.Api.Common;
using BlueOffice.Domain;
using BlueOffice.Infrastructure.Mail;

namespace BlueOffice.Api.Sessions;

/// <summary>A person offered by the @mention picker.</summary>
/// <param name="Source"><c>host</c> (the host's directory) or <c>collaborator</c> (in the session now).</param>
public sealed record MentionCandidate(string Id, string Name, string? Email, string? AvatarUrl, string Source);

/// <summary>A person named in a comment (only <see cref="Id"/> is trusted; the server looks the person up again).</summary>
public sealed record MentionTarget(string Id, string? Name, string? Email);

/// <summary>Body of <c>POST /api/sessions/{id}/mentions</c>: a comment with new mentions was posted.</summary>
/// <param name="CommentId">Comment id in the file.</param>
/// <param name="Text">Comment text.</param>
/// <param name="Mentions">People newly mentioned.</param>
/// <param name="Assignee">Mentioned person the comment assigns a task to.</param>
/// <param name="App"><c>doc</c>, <c>sheet</c> or <c>slide</c>.</param>
public sealed record CommentMentionRequest(string CommentId, string Text, List<MentionTarget>? Mentions, MentionTarget? Assignee, string? App);

/// <summary>What was sent for a <see cref="CommentMentionRequest"/>.</summary>
/// <param name="Recipients">People notified (verified, without the author).</param>
/// <param name="Host"><c>sent</c>, <c>unsupported</c> (WOPI / host without §A3), <c>failed</c> or <c>none</c> (nobody to notify).</param>
/// <param name="Emails">E-mails sent (0 when SMTP is not configured).</param>
public sealed record CommentMentionResult(int Recipients, string Host, int Emails);

/// <summary>
/// @mentions in comments (CORE-COMMENT-001): the people picker (host directory through BOHP §A3 + the session's
/// collaborators) and the notifications of a posted comment (host notification + optional SMTP e-mail).
/// </summary>
public sealed class CommentMentionService(SessionStore store, IMailSender mail, TimeProvider time, ILogger<CommentMentionService> logger)
{
    /// <summary>Most people returned by one search.</summary>
    public const int MaxResults = 20;

    /// <summary>Most people notified for one comment.</summary>
    public const int MaxRecipients = 20;

    /// <summary>People matching <paramref name="query"/> (name or e-mail, accents and case ignored).</summary>
    public async Task<IReadOnlyList<MentionCandidate>> SearchAsync(EditorSession s, string? query, int limit, CancellationToken ct)
    {
        var q = (query ?? "").Trim();
        limit = Math.Clamp(limit, 1, MaxResults);
        var list = new List<MentionCandidate>();
        foreach (var p in await HostPeopleAsync(s, q, limit, ct))
        {
            Add(list, new MentionCandidate(p.Id, p.Name, p.Email, p.AvatarUrl, "host"));
        }

        foreach (var u in Collaborators(s).Where(u => Matches(q, u.Name, null)))
        {
            Add(list, new MentionCandidate(u.Id, u.Name, null, u.AvatarUrl, "collaborator"));
        }

        return list.Take(limit).ToList();
    }

    /// <summary>Notifies the people of a posted comment; nothing is sent to unknown ids or to the author.</summary>
    public async Task<CommentMentionResult> NotifyAsync(EditorSession s, CommentMentionRequest body, CancellationToken ct)
    {
        if (!s.Permissions.Comment && !s.Permissions.Edit)
        {
            throw ApiException.Forbidden("Comment permission required.");
        }

        if (string.IsNullOrWhiteSpace(body.CommentId))
        {
            throw ApiException.BadRequest("commentId required.");
        }

        var wanted = (body.Mentions ?? []).Where(m => !string.IsNullOrWhiteSpace(m.Id)).ToList();
        if (body.Assignee is { } a && !string.IsNullOrWhiteSpace(a.Id) && wanted.All(m => m.Id != a.Id))
        {
            wanted.Add(a);
        }

        var people = new List<HostPerson>();
        foreach (var m in wanted.DistinctBy(m => m.Id).Take(MaxRecipients))
        {
            if (m.Id != s.User.Id && await ResolveAsync(s, m, ct) is { } p)
            {
                people.Add(p);
            }
        }

        if (people.Count == 0)
        {
            return new CommentMentionResult(0, "none", 0);
        }

        var text = Shorten(body.Text, 500);
        var assignee = body.Assignee is { } t ? people.FirstOrDefault(p => p.Id == t.Id) : null;
        var mentioned = people.Where(p => p != assignee).ToList();
        var host = "unsupported";
        if (s.Storage is IHostPeople hp)
        {
            host = "sent";
            List<HostPerson> assigned = assignee is null ? [] : [assignee];
            foreach (var (type, to) in new[] { ("mention", mentioned), ("task", assigned) })
            {
                if (to.Count == 0)
                {
                    continue;
                }

                try
                {
                    await hp.NotifyAsync(s.Context, s.FileId, new HostCommentNotification(type, body.CommentId, text, s.User, to, s.File.Name, body.App), ct);
                }
                catch (HostException e) when (e.StatusCode is 404 or 405 or 501)
                {
                    host = "unsupported";
                }
                catch (HostException e)
                {
                    logger.LogWarning("Comment notification to host {Host} failed: {Message}", s.Host.Name, e.Message);
                    host = "failed";
                }
            }
        }

        var emails = 0;
        if (mail.IsConfigured)
        {
            foreach (var p in people.Where(p => !string.IsNullOrWhiteSpace(p.Email)))
            {
                try
                {
                    await mail.SendAsync(Email(s, p, p == assignee, text), ct);
                    emails++;
                }
                catch (Exception e) when (e is System.Net.Mail.SmtpException or InvalidOperationException or FormatException or IOException)
                {
                    logger.LogWarning("Mention e-mail to {To} failed: {Message}", p.Email, e.Message);
                }
            }
        }

        return new CommentMentionResult(people.Count, host, emails);
    }

    private static OutgoingMail Email(EditorSession s, HostPerson to, bool task, string text)
    {
        var subject = task
            ? $"{s.User.Name} đã giao việc cho bạn trong \"{s.File.Name}\""
            : $"{s.User.Name} đã nhắc đến bạn trong \"{s.File.Name}\"";
        var en = task
            ? $"{s.User.Name} assigned you a task in \"{s.File.Name}\"."
            : $"{s.User.Name} mentioned you in \"{s.File.Name}\".";
        var link = string.IsNullOrEmpty(s.File.ShareUrl) ? "" : $"\n\n{s.File.ShareUrl}";
        return new OutgoingMail(to.Email!, to.Name, subject, $"{subject}:\n\n{text}\n\n{en}{link}\n");
    }

    /// <summary>The host's record of <paramref name="m"/>, or the collaborator with that id; null if unknown.</summary>
    private async Task<HostPerson?> ResolveAsync(EditorSession s, MentionTarget m, CancellationToken ct)
    {
        foreach (var q in new[] { m.Name, m.Email, m.Id })
        {
            if (string.IsNullOrWhiteSpace(q))
            {
                continue;
            }

            var hit = (await HostPeopleAsync(s, q, MaxResults, ct)).FirstOrDefault(p => p.Id == m.Id);
            if (hit is not null)
            {
                return hit;
            }
        }

        return Collaborators(s).FirstOrDefault(u => u.Id == m.Id) is { } c ? new HostPerson(c.Id, c.Name, null, c.AvatarUrl) : null;
    }

    private async Task<IReadOnlyList<HostPerson>> HostPeopleAsync(EditorSession s, string q, int limit, CancellationToken ct)
    {
        if (s.Storage is not IHostPeople hp)
        {
            return [];
        }

        try
        {
            return await hp.SearchPeopleAsync(s.Context, s.FileId, q, limit, ct);
        }
        catch (HostException e)
        {
            // Hosts without §A3 (404 / 501) or a failing directory: the picker still offers the collaborators.
            if (e.StatusCode is not (404 or 405 or 501))
            {
                logger.LogInformation("People search on host {Host} failed: {Message}", s.Host.Name, e.Message);
            }

            return [];
        }
    }

    /// <summary>Users with a live session on the same file (the room's collaborators).</summary>
    private IEnumerable<HostUser> Collaborators(EditorSession s)
    {
        var now = time.GetUtcNow().UtcDateTime;
        return store.All
            .Where(x => x.RoomKey == s.RoomKey && x.ExpiresAt > now && !string.IsNullOrEmpty(x.User.Id))
            .Select(x => x.User)
            .DistinctBy(u => u.Id);
    }

    private static void Add(List<MentionCandidate> list, MentionCandidate c)
    {
        if (list.All(x => x.Id != c.Id))
        {
            list.Add(c);
        }
    }

    /// <summary>Case- and accent-insensitive containment ("binh" finds "Trần Thị Bình").</summary>
    public static bool Matches(string query, string name, string? email) =>
        query.Length == 0
        || Fold(name).Contains(Fold(query), StringComparison.Ordinal)
        || (email is not null && Fold(email).Contains(Fold(query), StringComparison.Ordinal));

    private static string Fold(string s)
    {
        var sb = new StringBuilder(s.Length);
        foreach (var ch in s.Normalize(NormalizationForm.FormD))
        {
            if (CharUnicodeInfo.GetUnicodeCategory(ch) != UnicodeCategory.NonSpacingMark)
            {
                sb.Append(ch is 'đ' or 'Đ' ? 'd' : char.ToLowerInvariant(ch));
            }
        }

        return sb.ToString();
    }

    private static string Shorten(string? s, int max)
    {
        var t = (s ?? "").Trim();
        return t.Length <= max ? t : t[..max] + "…";
    }
}
