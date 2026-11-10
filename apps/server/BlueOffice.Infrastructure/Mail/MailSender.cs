using System.Net;
using System.Net.Mail;
using Microsoft.Extensions.Options;

namespace BlueOffice.Infrastructure.Mail;

/// <summary>
/// SMTP relay of the server (section <c>Mail</c>), shared by every server-sent e-mail: comment @mention / task e-mails
/// (CORE-COMMENT-001) and the server relay a mail-merge <c>MailProvider</c> can use. Nothing is ever sent while
/// <see cref="SmtpHost"/> or <see cref="From"/> is empty.
/// </summary>
public sealed class MailOptions
{
    /// <summary>SMTP server; empty = e-mail disabled.</summary>
    public string? SmtpHost { get; set; }

    /// <summary>SMTP port (587 submission with STARTTLS by default).</summary>
    public int SmtpPort { get; set; } = 587;

    /// <summary>Use TLS (STARTTLS on 587).</summary>
    public bool EnableSsl { get; set; } = true;

    /// <summary>SMTP user (empty: no authentication).</summary>
    public string? User { get; set; }

    /// <summary>SMTP password.</summary>
    public string? Password { get; set; }

    /// <summary>Sender address.</summary>
    public string? From { get; set; }

    /// <summary>Sender display name.</summary>
    public string FromName { get; set; } = "BlueOffice";

    /// <summary>True when e-mail can be sent.</summary>
    public bool IsConfigured => !string.IsNullOrWhiteSpace(SmtpHost) && !string.IsNullOrWhiteSpace(From);
}

/// <summary>An outgoing plain-text e-mail.</summary>
public sealed record OutgoingMail(string To, string? ToName, string Subject, string Body);

/// <summary>Sends e-mails through the configured relay.</summary>
public interface IMailSender
{
    /// <summary>False when no relay is configured: callers send nothing.</summary>
    bool IsConfigured { get; }

    /// <summary>Sends one e-mail.</summary>
    Task SendAsync(OutgoingMail mail, CancellationToken ct);
}

/// <summary><see cref="IMailSender"/> over <see cref="SmtpClient"/> (System.Net.Mail).</summary>
public sealed class SmtpMailSender(IOptionsMonitor<MailOptions> options) : IMailSender
{
    /// <inheritdoc />
    public bool IsConfigured => options.CurrentValue.IsConfigured;

    /// <inheritdoc />
    public async Task SendAsync(OutgoingMail mail, CancellationToken ct)
    {
        var o = options.CurrentValue;
        if (!o.IsConfigured)
        {
            return;
        }

        using var msg = new MailMessage
        {
            From = new MailAddress(o.From!, o.FromName),
            Subject = mail.Subject,
            Body = mail.Body,
            IsBodyHtml = false,
            BodyEncoding = System.Text.Encoding.UTF8,
            SubjectEncoding = System.Text.Encoding.UTF8,
        };
        msg.To.Add(new MailAddress(mail.To, mail.ToName));
        using var smtp = new SmtpClient(o.SmtpHost, o.SmtpPort) { EnableSsl = o.EnableSsl };
        if (!string.IsNullOrEmpty(o.User))
        {
            smtp.Credentials = new NetworkCredential(o.User, o.Password);
        }

        await smtp.SendMailAsync(msg, ct);
    }
}
