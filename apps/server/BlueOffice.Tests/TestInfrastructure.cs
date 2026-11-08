using System.Collections.Concurrent;
using System.Net.Http.Headers;
using System.Net.Http.Json;
using System.Net.WebSockets;
using System.Text;
using System.Text.Json;
using BlueOffice.Domain;
using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Mvc.Testing;
using Microsoft.AspNetCore.TestHost;
using Microsoft.Data.Sqlite;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.DependencyInjection.Extensions;
using Microsoft.IdentityModel.JsonWebTokens;
using Microsoft.IdentityModel.Tokens;
using MockBlueDrive;

namespace BlueOffice.Tests;

/// <summary>Controllable clock.</summary>
public sealed class TestClock : TimeProvider
{
    private DateTimeOffset _now = DateTimeOffset.UtcNow;

    /// <inheritdoc />
    public override DateTimeOffset GetUtcNow() => _now;

    /// <summary>Moves time forward.</summary>
    public void Advance(TimeSpan by) => _now += by;
}

/// <summary>Fake bo-cli: records calls and produces deterministic outputs.</summary>
public sealed class FakeRunner : IBoCliRunner
{
    /// <summary>Recorded argument lists.</summary>
    public ConcurrentQueue<IReadOnlyList<string>> Calls { get; } = new();

    /// <summary>Optional override.</summary>
    public Func<IReadOnlyList<string>, ProcessResult?>? Handler { get; set; }

    /// <summary>Pages written by <c>render</c> without <c>--page</c> (<c>%d</c> in <c>--out</c>).</summary>
    public int RenderPages { get; set; } = 2;

    /// <inheritdoc />
    public async Task<ProcessResult> RunAsync(IReadOnlyList<string> args, string workingDirectory, CancellationToken ct)
    {
        Calls.Enqueue(args);
        if (Handler?.Invoke(args) is { } custom)
        {
            return custom;
        }

        string Out() => args[args.ToList().IndexOf("--out") + 1];
        switch (args[0])
        {
            case "convert":
                await File.WriteAllBytesAsync(Out(), "%PDF-1.7 fake"u8.ToArray(), ct);
                return new ProcessResult(0, "", "");
            case "render":
                {
                    byte[] png = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
                    if (!Out().Contains("%d", StringComparison.Ordinal))
                    {
                        await File.WriteAllBytesAsync(Out(), png, ct);
                        return new ProcessResult(0, "", "");
                    }

                    // Every page: `RenderPages` pages, page number appended to the PNG signature.
                    for (var p = 1; p <= RenderPages; p++)
                    {
                        await File.WriteAllBytesAsync(Out().Replace("%d", p.ToString(System.Globalization.CultureInfo.InvariantCulture), StringComparison.Ordinal), [.. png, (byte)p], ct);
                    }

                    return new ProcessResult(0, "", "");
                }

            case "dump":
                var page = int.Parse(args[^1], System.Globalization.CultureInfo.InvariantCulture);
                return page switch
                {
                    0 => new ProcessResult(0, """{"commands":[{"op":"glyphs","y":10,"text":"Xin chào"},{"op":"rect"},{"op":"glyphs","y":10,"text":" thế giới"},{"op":"glyphs","y":30,"text":"Dòng hai"}]}""", ""),
                    1 => new ProcessResult(0, """{"commands":[{"op":"glyphs","y":10,"text":"Trang 2"}]}""", ""),
                    _ => new ProcessResult(1, "", $"error: page {page + 1} out of range"),
                };
            default:
                return new ProcessResult(1, "", "unknown");
        }
    }
}

/// <summary>Mock BlueDrive running on an in-memory TestServer.</summary>
public sealed class MockDrive : IAsyncDisposable
{
    private MockDrive(WebApplication app)
    {
        App = app;
        Server = app.GetTestServer();
        Client = Server.CreateClient();
    }

    /// <summary>App.</summary>
    public WebApplication App { get; }

    /// <summary>Server.</summary>
    public TestServer Server { get; }

    /// <summary>Client for admin calls.</summary>
    public HttpClient Client { get; }

    /// <summary>Starts the mock.</summary>
    public static async Task<MockDrive> StartAsync()
    {
        var app = MockBlueDriveApp.Build([], b => b.WebHost.UseTestServer());
        await app.StartAsync();
        return new MockDrive(app);
    }

    /// <summary>Issues a BlueDrive token.</summary>
    public async Task<string> TokenAsync(string userId, string? fileId = null, int ttl = 600)
    {
        var r = await Client.PostAsJsonAsync("/_admin/token", new TokenRequest(userId, fileId, ttl));
        r.EnsureSuccessStatusCode();
        return (await r.Content.ReadFromJsonAsync<JsonElement>()).GetProperty("accessToken").GetString()!;
    }

    /// <summary>Creates a file owned by u1.</summary>
    public async Task<string> CreateFileAsync(string name, string content)
    {
        var r = await Client.PostAsJsonAsync("/_admin/files", new CreateFileRequest(name, "u1", Convert.ToBase64String(Encoding.UTF8.GetBytes(content)), null));
        r.EnsureSuccessStatusCode();
        return (await r.Content.ReadFromJsonAsync<JsonElement>()).GetProperty("id").GetString()!;
    }

    /// <summary>Sets permissions of a user on a file.</summary>
    public async Task SetPermsAsync(string fileId, string userId, bool view, bool edit)
    {
        var r = await Client.PutAsJsonAsync($"/_admin/files/{fileId}/permissions/{userId}", new Perms(view, edit, edit, false, view));
        r.EnsureSuccessStatusCode();
    }

    /// <summary>Admin view of a file.</summary>
    public async Task<JsonElement> FileStateAsync(string fileId) =>
        await Client.GetFromJsonAsync<JsonElement>($"/_admin/files/{fileId}");

    /// <summary>Webhooks received.</summary>
    public async Task<List<WebhookRecord>> WebhooksAsync() =>
        (await Client.GetFromJsonAsync<List<WebhookRecord>>("/_admin/webhooks"))!;

    /// <inheritdoc />
    public async ValueTask DisposeAsync()
    {
        Client.Dispose();
        await App.StopAsync();
        await App.DisposeAsync();
    }
}

/// <summary>BlueOffice API under test: SQLite temp file, file-system object store, dev JWT key.</summary>
public sealed class TestApp : WebApplicationFactory<Program>
{
    /// <summary>Dev signing key used by tests.</summary>
    public const string DevKey = "test-signing-key-0123456789-abcdefghijklmnop";

    private readonly string _dir = Directory.CreateTempSubdirectory("bo-test-").FullName;
    private readonly Dictionary<string, string?> _settings;
    private readonly MockDrive? _drive;
    private readonly TestServer? _wopi;
    private readonly TimeProvider? _clock;

    /// <summary>Creates the app.</summary>
    public TestApp(MockDrive? drive = null, TestServer? wopi = null, Dictionary<string, string?>? settings = null, TimeProvider? clock = null)
    {
        _drive = drive;
        _wopi = wopi;
        _clock = clock;
        _settings = new Dictionary<string, string?>
        {
            ["Database:Provider"] = "Sqlite",
            ["ConnectionStrings:Default"] = $"Data Source={Path.Combine(_dir, "db.sqlite")}",
            ["Storage:Root"] = Path.Combine(_dir, "objects"),
            ["Auth:DevSigningKey"] = DevKey,
            ["Auth:DevIssuer"] = "blueoffice-dev",
            ["Auth:Audience"] = "blueoffice-api",
            ["Otel:Enabled"] = "false",
            ["Collab:TickInterval"] = "00:00:00.100",
            ["Collab:AutosaveDebounce"] = "00:00:00",
            ["Hosts:0:Name"] = "bluedrive",
            ["Hosts:0:Protocol"] = "Bohp",
            ["Hosts:0:BaseUrl"] = "http://mock/bohp/v1",
            ["Hosts:0:JwksUrl"] = "http://mock/.well-known/jwks.json",
            ["Hosts:0:Issuer"] = "http://mock-bluedrive",
            ["Hosts:0:Audience"] = "blueoffice",
            ["Hosts:0:WebhookUrl"] = "http://mock/webhooks/blueoffice",
            ["Hosts:0:WebhookSecret"] = "mock-webhook-secret",
            ["Hosts:0:AllowedOrigins:0"] = "https://drive.mock",
            ["Hosts:1:Name"] = "wopihost",
            ["Hosts:1:Protocol"] = "Wopi",
            ["Hosts:1:BaseUrl"] = "http://wopi/wopi/files",
        };
        foreach (var kv in settings ?? [])
        {
            _settings[kv.Key] = kv.Value;
        }
    }

    /// <summary>Fake bo-cli.</summary>
    public FakeRunner Runner { get; } = new();

    /// <summary>Object store root.</summary>
    public string ObjectsRoot => Path.Combine(_dir, "objects");

    /// <summary>A dev JWT for a user.</summary>
    public static string UserToken(string sub, string name)
    {
        var key = new SymmetricSecurityKey(Encoding.UTF8.GetBytes(DevKey));
        return new JsonWebTokenHandler().CreateToken(new SecurityTokenDescriptor
        {
            Issuer = "blueoffice-dev",
            Audience = "blueoffice-api",
            Claims = new Dictionary<string, object> { ["sub"] = sub, ["name"] = name, ["email"] = sub + "@example.vn" },
            Expires = DateTime.UtcNow.AddHours(1),
            SigningCredentials = new SigningCredentials(key, SecurityAlgorithms.HmacSha256),
        });
    }

    /// <summary>HTTP client authenticated as <paramref name="sub"/>.</summary>
    public HttpClient ClientFor(string sub, string? name = null)
    {
        var c = CreateClient();
        c.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Bearer", UserToken(sub, name ?? sub));
        return c;
    }

    /// <summary>Opens a WebSocket.</summary>
    public Task<WsClient> ConnectAsync(string pathAndQuery) => WsClient.ConnectAsync(this, pathAndQuery);

    /// <inheritdoc />
    protected override void ConfigureWebHost(IWebHostBuilder builder)
    {
        builder.UseEnvironment("Development");
        foreach (var kv in _settings)
        {
            builder.UseSetting(kv.Key, kv.Value);
        }

        builder.ConfigureServices(s =>
        {
            // A configured BoCli:ExecutablePath means "run the real bo-cli" (BoCliEndToEndTests).
            if (!_settings.ContainsKey("BoCli:ExecutablePath"))
            {
                s.RemoveAll<IBoCliRunner>();
                s.AddSingleton<IBoCliRunner>(Runner);
            }

            if (_clock is not null)
            {
                s.RemoveAll<TimeProvider>();
                s.AddSingleton(_clock);
            }

            if (_drive is not null)
            {
                foreach (var name in new[] { "bohp", "jwks", "webhooks" })
                {
                    s.AddHttpClient(name).ConfigurePrimaryHttpMessageHandler(() => _drive.Server.CreateHandler());
                }
            }

            if (_wopi is not null)
            {
                s.AddHttpClient("wopi").ConfigurePrimaryHttpMessageHandler(() => _wopi.CreateHandler());
            }
        });
    }

    /// <inheritdoc />
    protected override void Dispose(bool disposing)
    {
        base.Dispose(disposing);
        SqliteConnection.ClearAllPools();
        try
        {
            Directory.Delete(_dir, recursive: true);
        }
        catch (IOException)
        {
            // Best effort.
        }
        catch (UnauthorizedAccessException)
        {
            // Best effort.
        }
    }
}

/// <summary>A received WebSocket message.</summary>
public sealed record WsMessage(WebSocketMessageType Type, byte[] Data)
{
    /// <summary>Text payload.</summary>
    public string Text => Encoding.UTF8.GetString(Data);

    /// <summary>JSON "type" of a text message.</summary>
    public string? EventType => Type == WebSocketMessageType.Text ? JsonDocument.Parse(Data).RootElement.GetProperty("type").GetString() : null;

    /// <summary>Parsed JSON.</summary>
    public JsonElement Json => JsonDocument.Parse(Data).RootElement.Clone();
}

/// <summary>Test WebSocket client recording every received message.</summary>
public sealed class WsClient : IAsyncDisposable
{
    private readonly List<WsMessage> _messages = [];
    private readonly HashSet<WsMessage> _consumed = [];
    private readonly Task _loop;

    private WsClient(WebSocket socket)
    {
        Socket = socket;
        _loop = Task.Run(ReceiveLoopAsync);
    }

    /// <summary>Socket.</summary>
    public WebSocket Socket { get; }

    /// <summary>Close status received from the server.</summary>
    public WebSocketCloseStatus? CloseStatus { get; private set; }

    /// <summary>All messages received so far.</summary>
    public IReadOnlyList<WsMessage> Messages
    {
        get
        {
            lock (_messages)
            {
                return _messages.ToList();
            }
        }
    }

    /// <summary>Connects.</summary>
    public static async Task<WsClient> ConnectAsync(TestApp app, string pathAndQuery)
    {
        var ws = app.Server.CreateWebSocketClient();
        var uri = new Uri(new Uri("ws://localhost"), pathAndQuery);
        var socket = await ws.ConnectAsync(uri, CancellationToken.None);
        return new WsClient(socket);
    }

    private async Task ReceiveLoopAsync()
    {
        var buffer = new byte[1 << 20];
        try
        {
            while (Socket.State == WebSocketState.Open)
            {
                using var ms = new MemoryStream();
                WebSocketReceiveResult r;
                do
                {
                    r = await Socket.ReceiveAsync(buffer, CancellationToken.None);
                    ms.Write(buffer, 0, r.Count);
                }
                while (!r.EndOfMessage);

                if (r.MessageType == WebSocketMessageType.Close)
                {
                    CloseStatus = r.CloseStatus;
                    return;
                }

                lock (_messages)
                {
                    _messages.Add(new WsMessage(r.MessageType, ms.ToArray()));
                }
            }
        }
        catch (Exception e) when (e is WebSocketException or OperationCanceledException or IOException)
        {
            // Closed.
        }
    }

    /// <summary>Sends a binary frame.</summary>
    public Task SendAsync(byte[] data) => Socket.SendAsync(data, WebSocketMessageType.Binary, true, CancellationToken.None);

    /// <summary>Sends a JSON control message.</summary>
    public Task SendJsonAsync(object payload) =>
        Socket.SendAsync(JsonSerializer.SerializeToUtf8Bytes(payload), WebSocketMessageType.Text, true, CancellationToken.None);

    /// <summary>Waits for (and consumes) the first unconsumed message matching <paramref name="match"/>.</summary>
    public async Task<WsMessage> WaitAsync(Func<WsMessage, bool> match, string what, int timeoutMs = 5000)
    {
        var deadline = DateTime.UtcNow.AddMilliseconds(timeoutMs);
        while (DateTime.UtcNow < deadline)
        {
            lock (_messages)
            {
                var m = _messages.FirstOrDefault(x => !_consumed.Contains(x) && match(x));
                if (m is not null)
                {
                    _consumed.Add(m);
                    return m;
                }
            }

            await Task.Delay(20);
        }

        throw new TimeoutException($"Timed out waiting for {what}. Received: {string.Join(" | ", Messages.Select(Describe))}");
    }

    /// <summary>Waits for a JSON event of the given type.</summary>
    public async Task<JsonElement> WaitEventAsync(string type, int timeoutMs = 5000) =>
        (await WaitAsync(m => m.EventType == type, $"event '{type}'", timeoutMs)).Json;

    /// <summary>Waits for a binary frame equal to <paramref name="expected"/>.</summary>
    public Task<WsMessage> WaitBinaryAsync(byte[] expected, int timeoutMs = 5000) =>
        WaitAsync(m => m.Type == WebSocketMessageType.Binary && m.Data.SequenceEqual(expected), Convert.ToHexString(expected), timeoutMs);

    /// <summary>Waits until the server closed the socket.</summary>
    public async Task WaitClosedAsync(int timeoutMs = 5000) => await _loop.WaitAsync(TimeSpan.FromMilliseconds(timeoutMs));

    private static string Describe(WsMessage m) => m.Type == WebSocketMessageType.Text ? m.Text : Convert.ToHexString(m.Data);

    /// <inheritdoc />
    public async ValueTask DisposeAsync()
    {
        if (Socket.State == WebSocketState.Open)
        {
            try
            {
                await Socket.CloseOutputAsync(WebSocketCloseStatus.NormalClosure, "", CancellationToken.None);
            }
            catch (Exception e) when (e is WebSocketException or IOException or OperationCanceledException)
            {
                // Ignore.
            }
        }

        await _loop.WaitAsync(TimeSpan.FromSeconds(5)).ContinueWith(_ => { }, TaskScheduler.Default);
        Socket.Dispose();
    }
}
