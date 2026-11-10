using System.Net;
using System.Net.Http.Headers;
using System.Net.Http.Json;
using System.Text;
using System.Text.Json;
using BlueOffice.Domain;

namespace BlueOffice.Infrastructure.Hosts;

/// <summary>CheckFileInfo response subset ([MS-WOPI] §3.3.5.1.1.2).</summary>
public sealed class WopiFileInfo
{
    /// <summary>File name with extension.</summary>
    public string BaseFileName { get; set; } = "";

    /// <summary>Owner id.</summary>
    public string? OwnerId { get; set; }

    /// <summary>Size in bytes.</summary>
    public long Size { get; set; }

    /// <summary>Current version ([MS-WOPI] §3.3.5.1.1.2 Version: a string; numbers sent by some hosts are accepted).</summary>
    [System.Text.Json.Serialization.JsonConverter(typeof(StringOrNumberConverter))]
    public string? Version { get; set; }

    /// <summary>Current user id.</summary>
    public string? UserId { get; set; }

    /// <summary>Current user name.</summary>
    public string? UserFriendlyName { get; set; }

    /// <summary>User may write.</summary>
    public bool UserCanWrite { get; set; }

    /// <summary>File is read-only for everyone.</summary>
    public bool ReadOnly { get; set; }

    /// <summary>Host supports Lock/Unlock/RefreshLock.</summary>
    public bool SupportsLocks { get; set; }

    /// <summary>Host supports PutFile.</summary>
    public bool SupportsUpdate { get; set; }

    /// <summary>Host supports RenameFile.</summary>
    public bool SupportsRename { get; set; }

    /// <summary>User may rename.</summary>
    public bool UserCanRename { get; set; }

    /// <summary>User may not create files with PutRelativeFile.</summary>
    public bool UserCanNotWriteRelative { get; set; }

    /// <summary>Host disables export/download.</summary>
    public bool DisableExport { get; set; }

    /// <summary>Host disables print.</summary>
    public bool DisablePrint { get; set; }

    /// <summary>Sharing UI URL.</summary>
    public string? FileSharingUrl { get; set; }

    /// <summary>Last modification (ISO 8601).</summary>
    public string? LastModifiedTime { get; set; }
}

/// <summary>Reads a JSON string, number or boolean as a string.</summary>
public sealed class StringOrNumberConverter : System.Text.Json.Serialization.JsonConverter<string?>
{
    /// <inheritdoc />
    public override string? Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options) => reader.TokenType switch
    {
        JsonTokenType.String => reader.GetString(),
        JsonTokenType.Number => Encoding.UTF8.GetString(reader.ValueSpan),
        JsonTokenType.True => "true",
        JsonTokenType.False => "false",
        JsonTokenType.Null => null,
        _ => throw new JsonException("Expected a string or a number."),
    };

    /// <inheritdoc />
    public override void Write(Utf8JsonWriter writer, string? value, JsonSerializerOptions options)
    {
        if (value is null)
        {
            writer.WriteNullValue();
        }
        else
        {
            writer.WriteStringValue(value);
        }
    }
}

/// <summary>WOPI-client adapter: BlueOffice calls a WOPI host's files endpoint ([MS-WOPI] §3.3.5).</summary>
public interface IWopiClient : IHostStorage
{
    /// <summary>Raw CheckFileInfo.</summary>
    Task<WopiFileInfo> CheckFileInfoAsync(HostCallContext ctx, string fileId, CancellationToken ct);
}

/// <summary>
/// HTTP implementation of <see cref="IWopiClient"/>. WOPISrc = <c>{BaseUrl}/{fileId}</c>, the access token travels in the
/// <c>access_token</c> query parameter ([MS-WOPI] §3.3.1). Version history and restore are not part of WOPI.
/// Every request is signed with the proof keys (<see cref="WopiProofKeys"/>): <c>X-WOPI-Proof</c>, <c>X-WOPI-ProofOld</c>,
/// <c>X-WOPI-TimeStamp</c>.
/// </summary>
public sealed class WopiClient(IHttpClientFactory httpFactory, WopiProofKeys proofKeys, TimeProvider time) : IWopiClient
{
    /// <summary>Name of the HttpClient.</summary>
    public const string HttpClientName = "wopi";

    private static readonly JsonSerializerOptions Json = new() { PropertyNameCaseInsensitive = true };

    /// <inheritdoc />
    public HostProtocol Protocol => HostProtocol.Wopi;

    /// <inheritdoc />
    public async Task<WopiFileInfo> CheckFileInfoAsync(HostCallContext ctx, string fileId, CancellationToken ct)
    {
        // [MS-WOPI] §3.3.5.1.1 CheckFileInfo: GET /wopi/files/(file_id)
        using var res = await SendAsync(ctx, HttpMethod.Get, fileId, "", null, null, ct);
        return await res.Content.ReadFromJsonAsync<WopiFileInfo>(Json, ct) ?? throw new HostException(502, "Empty CheckFileInfo.");
    }

    /// <inheritdoc />
    public async Task<HostFileInfo> GetFileAsync(HostCallContext ctx, string fileId, CancellationToken ct)
    {
        var i = await CheckFileInfoAsync(ctx, fileId, ct);
        var canWrite = i.UserCanWrite && !i.ReadOnly && i.SupportsUpdate;
        DateTime? modified = DateTime.TryParse(i.LastModifiedTime, System.Globalization.CultureInfo.InvariantCulture,
            System.Globalization.DateTimeStyles.AdjustToUniversal | System.Globalization.DateTimeStyles.AssumeUniversal, out var m) ? m : null;
        return new HostFileInfo(
            fileId,
            i.BaseFileName,
            i.Size,
            i.Version ?? "",
            i.OwnerId,
            new HostPermissions(View: true, Comment: canWrite, Edit: canWrite, Share: i.FileSharingUrl is not null, Download: !i.DisableExport),
            new HostUser(i.UserId ?? "", i.UserFriendlyName ?? i.UserId ?? "", null),
            i.FileSharingUrl,
            modified,
            LockId: null,
            SupportsLocks: i.SupportsLocks,
            SupportsVersions: false);
    }

    /// <inheritdoc />
    public async Task<HostContent> GetContentsAsync(HostCallContext ctx, string fileId, CancellationToken ct)
    {
        // [MS-WOPI] §3.3.5.3.1 GetFile: GET /wopi/files/(file_id)/contents
        using var res = await SendAsync(ctx, HttpMethod.Get, fileId, "/contents", null, null, ct);
        return new HostContent(await res.Content.ReadAsByteArrayAsync(ct), Header(res, "X-WOPI-ItemVersion"));
    }

    /// <inheritdoc />
    public async Task<HostPutResult> PutContentsAsync(HostCallContext ctx, string fileId, byte[] content, HostPutOptions options, CancellationToken ct)
    {
        if (options.IfMatchVersion is { } expected)
        {
            // WOPI has no If-Match: compare with CheckFileInfo.Version first (best effort, the lock closes the race).
            var current = (await CheckFileInfoAsync(ctx, fileId, ct)).Version;
            if (current is not null && current != expected)
            {
                throw new HostConflictException("version", current, null);
            }
        }

        // [MS-WOPI] §3.3.5.3.2 PutFile: POST /wopi/files/(file_id)/contents, X-WOPI-Override: PUT
        using var body = new ByteArrayContent(content);
        body.Headers.ContentType = new MediaTypeHeaderValue("application/octet-stream");
        using var res = await SendAsync(ctx, HttpMethod.Post, fileId, "/contents", body, req =>
        {
            req.Headers.Add("X-WOPI-Override", "PUT");
            if (options.LockId is { } l)
            {
                req.Headers.Add("X-WOPI-Lock", l);
            }

            if (options.Editors.Count > 0)
            {
                req.Headers.Add("X-WOPI-Editors", string.Join(',', options.Editors));
            }
        }, ct);
        var version = Header(res, "X-WOPI-ItemVersion") ?? (await CheckFileInfoAsync(ctx, fileId, ct)).Version ?? "";
        return new HostPutResult(version);
    }

    /// <inheritdoc />
    public async Task<HostLockResult> LockAsync(HostCallContext ctx, string fileId, string lockId, TimeSpan duration, CancellationToken ct)
    {
        // [MS-WOPI] §3.3.5.1.3 Lock: X-WOPI-Override: LOCK. Lock expiry is fixed by the host (30 min).
        using var res = await OverrideAsync(ctx, fileId, "LOCK", r => r.Headers.Add("X-WOPI-Lock", lockId), ct);
        return new HostLockResult(lockId, null);
    }

    /// <inheritdoc />
    public async Task UnlockAsync(HostCallContext ctx, string fileId, string lockId, CancellationToken ct)
    {
        // [MS-WOPI] §3.3.5.1.6 Unlock: X-WOPI-Override: UNLOCK
        using var res = await OverrideAsync(ctx, fileId, "UNLOCK", r => r.Headers.Add("X-WOPI-Lock", lockId), ct);
    }

    /// <inheritdoc />
    public async Task<HostLockResult> RefreshLockAsync(HostCallContext ctx, string fileId, string lockId, TimeSpan duration, CancellationToken ct)
    {
        // [MS-WOPI] §3.3.5.1.5 RefreshLock: X-WOPI-Override: REFRESH_LOCK
        using var res = await OverrideAsync(ctx, fileId, "REFRESH_LOCK", r => r.Headers.Add("X-WOPI-Lock", lockId), ct);
        return new HostLockResult(lockId, null);
    }

    /// <inheritdoc />
    public Task<IReadOnlyList<HostVersion>> GetVersionsAsync(HostCallContext ctx, string fileId, CancellationToken ct) =>
        throw new HostNotSupportedException("versions");

    /// <inheritdoc />
    public Task<HostContent> GetVersionContentsAsync(HostCallContext ctx, string fileId, string version, CancellationToken ct) =>
        throw new HostNotSupportedException("versions");

    /// <inheritdoc />
    public Task<HostPutResult> RestoreVersionAsync(HostCallContext ctx, string fileId, string version, CancellationToken ct) =>
        throw new HostNotSupportedException("restore");

    /// <inheritdoc />
    public async Task<string> RenameAsync(HostCallContext ctx, string fileId, string newName, CancellationToken ct)
    {
        // [MS-WOPI] §3.3.5.1.9 RenameFile: X-WOPI-RequestedName is UTF-7 encoded and excludes the extension.
        var ext = Path.GetExtension(newName);
        var stem = Path.GetFileNameWithoutExtension(newName);
        using var res = await OverrideAsync(ctx, fileId, "RENAME_FILE", r => r.Headers.Add("X-WOPI-RequestedName", Utf7.Encode(stem)), ct);
        var dto = await res.Content.ReadFromJsonAsync<NameDto>(Json, ct);
        return (dto?.Name ?? stem) + ext;
    }

    /// <summary>
    /// [MS-WOPI] §3.3.5.1.8 PutRelativeFile (X-WOPI-Override: PUT_RELATIVE). WOPI creates the file next to an existing
    /// one, so <paramref name="folderId"/> is interpreted as the id of a sibling file.
    /// </summary>
    public async Task<HostCreatedFile> CreateFileAsync(HostCallContext ctx, string folderId, string name, byte[] content, CancellationToken ct)
    {
        using var body = new ByteArrayContent(content);
        body.Headers.ContentType = new MediaTypeHeaderValue("application/octet-stream");
        using var res = await SendAsync(ctx, HttpMethod.Post, folderId, "", body, r =>
        {
            r.Headers.Add("X-WOPI-Override", "PUT_RELATIVE");
            r.Headers.Add("X-WOPI-SuggestedTarget", Utf7.Encode(name));
            r.Headers.Add("X-WOPI-Size", content.Length.ToString(System.Globalization.CultureInfo.InvariantCulture));
        }, ct);
        var dto = await res.Content.ReadFromJsonAsync<RelativeDto>(Json, ct) ?? new RelativeDto();
        var id = "";
        if (Uri.TryCreate(dto.Url, UriKind.Absolute, out var u))
        {
            id = Uri.UnescapeDataString(u.AbsolutePath.TrimEnd('/').Split('/')[^1]);
        }

        return new HostCreatedFile(id, dto.Name ?? name, "");
    }

    private Task<HttpResponseMessage> OverrideAsync(HostCallContext ctx, string fileId, string op, Action<HttpRequestMessage> configure, CancellationToken ct) =>
        SendAsync(ctx, HttpMethod.Post, fileId, "", null, r =>
        {
            r.Headers.Add("X-WOPI-Override", op);
            configure(r);
        }, ct);

    private static string? Header(HttpResponseMessage res, string name) =>
        res.Headers.TryGetValues(name, out var v) ? v.FirstOrDefault() : null;

    private async Task<HttpResponseMessage> SendAsync(
        HostCallContext ctx,
        HttpMethod method,
        string fileId,
        string suffix,
        HttpContent? body,
        Action<HttpRequestMessage>? configure,
        CancellationToken ct)
    {
        var url = $"{ctx.Host.BaseUrl.TrimEnd('/')}/{Uri.EscapeDataString(fileId)}{suffix}?access_token={Uri.EscapeDataString(ctx.AccessToken)}";
        using var req = new HttpRequestMessage(method, new Uri(url)) { Content = body };
        configure?.Invoke(req);

        // Proof keys ([MS-WOPI] "Verifying that requests originate from the WOPI client"): signed over token + URL + time.
        // The URL is the one sent on the wire (System.Uri may normalise escapes of the composed string).
        var timestamp = time.GetUtcNow().UtcDateTime.Ticks;
        var (proof, proofOld) = proofKeys.Sign(ctx.AccessToken, req.RequestUri!.AbsoluteUri, timestamp);
        req.Headers.Add(WopiProof.ProofHeader, proof);
        req.Headers.Add(WopiProof.ProofOldHeader, proofOld);
        req.Headers.Add(WopiProof.TimeStampHeader, timestamp.ToString(System.Globalization.CultureInfo.InvariantCulture));
        HttpResponseMessage res;
        try
        {
            res = await httpFactory.CreateClient(HttpClientName).SendAsync(req, ct);
        }
        catch (HttpRequestException e)
        {
            throw new HostException(0, $"WOPI host '{ctx.Host.Name}' unreachable: {e.Message}");
        }

        if (res.IsSuccessStatusCode)
        {
            return res;
        }

        using (res)
        {
            if (res.StatusCode == HttpStatusCode.Conflict)
            {
                // 409: lock mismatch; X-WOPI-Lock carries the current lock ([MS-WOPI] §3.3.5.1.3).
                throw new HostConflictException("lock", null, Header(res, "X-WOPI-Lock"));
            }

            if (res.StatusCode == HttpStatusCode.NotImplemented)
            {
                throw new HostNotSupportedException(Header(res, "X-WOPI-Override") ?? method.Method);
            }

            throw new HostException((int)res.StatusCode, $"WOPI host '{ctx.Host.Name}' returned {(int)res.StatusCode}.");
        }
    }

    private sealed class NameDto
    {
        public string? Name { get; set; }
    }

    private sealed class RelativeDto
    {
        public string? Name { get; set; }

        public string? Url { get; set; }
    }
}

/// <summary>UTF-7 encoder (RFC 2152) used by WOPI headers X-WOPI-RequestedName / X-WOPI-SuggestedTarget.</summary>
public static class Utf7
{
    private const string Base64Chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    // RFC 2152 Set D (directly encoded) + space.
    private static bool IsDirect(char c) =>
        c is (>= 'A' and <= 'Z') or (>= 'a' and <= 'z') or (>= '0' and <= '9')
            or '\'' or '(' or ')' or ',' or '-' or '.' or '/' or ':' or '?' or ' ';

    /// <summary>Encodes <paramref name="s"/> as UTF-7.</summary>
    public static string Encode(string s)
    {
        var sb = new StringBuilder();
        var i = 0;
        while (i < s.Length)
        {
            var c = s[i];
            if (IsDirect(c))
            {
                sb.Append(c);
                i++;
            }
            else if (c == '+')
            {
                sb.Append("+-");
                i++;
            }
            else
            {
                var start = i;
                while (i < s.Length && !IsDirect(s[i]) && s[i] != '+')
                {
                    i++;
                }

                var bytes = Encoding.BigEndianUnicode.GetBytes(s[start..i]);
                sb.Append('+');
                int bits = 0, buffer = 0;
                foreach (var b in bytes)
                {
                    buffer = (buffer << 8) | b;
                    bits += 8;
                    while (bits >= 6)
                    {
                        bits -= 6;
                        sb.Append(Base64Chars[(buffer >> bits) & 0x3F]);
                    }
                }

                if (bits > 0)
                {
                    sb.Append(Base64Chars[(buffer << (6 - bits)) & 0x3F]);
                }

                // Explicit terminator is always allowed and keeps the decoder unambiguous.
                sb.Append('-');
            }
        }

        return sb.ToString();
    }
}
