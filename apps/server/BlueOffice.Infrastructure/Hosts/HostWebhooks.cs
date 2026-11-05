using System.Globalization;
using System.Net.Http.Headers;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Threading.Channels;
using BlueOffice.Domain;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging;

namespace BlueOffice.Infrastructure.Hosts;

/// <summary>Event names sent to hosts.</summary>
public static class HostEvents
{
    /// <summary>A new version was written to the host.</summary>
    public const string FileSaved = "file.saved";

    /// <summary>First participant joined a collaborative session.</summary>
    public const string SessionStarted = "session.started";

    /// <summary>Last participant left.</summary>
    public const string SessionEnded = "session.ended";

    /// <summary>Audit record (open/edit/export/print/download).</summary>
    public const string Audit = "audit";
}

/// <summary>Queues webhook notifications to hosts (fire-and-forget, retried in the background).</summary>
public interface IHostNotifier
{
    /// <summary>Queues <paramref name="eventName"/> for <paramref name="host"/> (ignored if the host has no WebhookUrl).</summary>
    void Publish(HostConfig host, string eventName, string fileId, object data);
}

/// <summary>A queued webhook.</summary>
public sealed record WebhookMessage(HostConfig Host, string Event, string FileId, object Data, DateTime At);

/// <summary>
/// Sends webhooks as JSON <c>{"event","host","fileId","at","data"}</c>, signed with
/// <c>X-BlueOffice-Signature: sha256=hex(HMAC-SHA256(secret, timestamp + "." + body))</c>.
/// </summary>
public sealed class HostWebhookDispatcher(IHttpClientFactory httpFactory, TimeProvider time, ILogger<HostWebhookDispatcher> logger)
    : BackgroundService, IHostNotifier
{
    /// <summary>Name of the HttpClient.</summary>
    public const string HttpClientName = "webhooks";

    private const int MaxAttempts = 3;
    private static readonly JsonSerializerOptions Json = new(JsonSerializerDefaults.Web);
    private readonly Channel<WebhookMessage> _queue = Channel.CreateBounded<WebhookMessage>(
        new BoundedChannelOptions(10_000) { FullMode = BoundedChannelFullMode.DropOldest });

    /// <inheritdoc />
    public void Publish(HostConfig host, string eventName, string fileId, object data)
    {
        if (string.IsNullOrEmpty(host.WebhookUrl))
        {
            return;
        }

        _queue.Writer.TryWrite(new WebhookMessage(host, eventName, fileId, data, time.GetUtcNow().UtcDateTime));
    }

    /// <summary>Computes the signature header value.</summary>
    public static string Sign(string secret, string timestamp, byte[] body)
    {
        var payload = Encoding.UTF8.GetBytes(timestamp + ".").Concat(body).ToArray();
        return "sha256=" + Convert.ToHexString(HMACSHA256.HashData(Encoding.UTF8.GetBytes(secret), payload)).ToLowerInvariant();
    }

    /// <inheritdoc />
    protected override async Task ExecuteAsync(CancellationToken stoppingToken)
    {
        await foreach (var msg in _queue.Reader.ReadAllAsync(stoppingToken))
        {
            for (var attempt = 1; attempt <= MaxAttempts; attempt++)
            {
                try
                {
                    if (await SendAsync(msg, stoppingToken))
                    {
                        break;
                    }
                }
                catch (HttpRequestException e)
                {
                    logger.LogWarning("Webhook {Event} to {Host} failed: {Error}", msg.Event, msg.Host.Name, e.Message);
                }

                if (attempt < MaxAttempts)
                {
                    await Task.Delay(TimeSpan.FromSeconds(attempt * attempt), stoppingToken);
                }
            }
        }
    }

    private async Task<bool> SendAsync(WebhookMessage msg, CancellationToken ct)
    {
        var body = JsonSerializer.SerializeToUtf8Bytes(
            new { @event = msg.Event, host = msg.Host.Name, fileId = msg.FileId, at = msg.At, data = msg.Data }, Json);
        using var req = new HttpRequestMessage(HttpMethod.Post, new Uri(msg.Host.WebhookUrl!));
        req.Content = new ByteArrayContent(body);
        req.Content.Headers.ContentType = new MediaTypeHeaderValue("application/json");
        var ts = time.GetUtcNow().ToUnixTimeSeconds().ToString(CultureInfo.InvariantCulture);
        req.Headers.Add("X-BlueOffice-Event", msg.Event);
        req.Headers.Add("X-BlueOffice-Timestamp", ts);
        if (!string.IsNullOrEmpty(msg.Host.WebhookSecret))
        {
            req.Headers.Add("X-BlueOffice-Signature", Sign(msg.Host.WebhookSecret, ts, body));
        }

        using var res = await httpFactory.CreateClient(HttpClientName).SendAsync(req, ct);
        if (!res.IsSuccessStatusCode)
        {
            logger.LogWarning("Webhook {Event} to {Host} returned {Status}", msg.Event, msg.Host.Name, (int)res.StatusCode);
        }

        // 4xx (except 429) will not get better by retrying.
        return res.IsSuccessStatusCode || ((int)res.StatusCode is >= 400 and < 500 and not 429);
    }
}
