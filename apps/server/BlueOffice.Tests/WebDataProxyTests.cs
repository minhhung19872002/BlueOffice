using System.Net;
using System.Net.Http.Json;
using System.Text;
using Xunit;

namespace BlueOffice.Tests;

/// <summary>The web page proxy of BlueSheet's "From Web" queries (SHEET-PQ-001).</summary>
public sealed class WebDataProxyTests
{
    private static readonly Uri Fetch = new("/api/data/web", UriKind.Relative);

    /// <summary>A local HTTP server answering a page and a 404.</summary>
    private static (HttpListener Listener, string Base) Serve(byte[] page)
    {
        var port = Random.Shared.Next(20000, 60000);
        var l = new HttpListener();
        var b = $"http://127.0.0.1:{port}/";
        l.Prefixes.Add(b);
        l.Start();
        _ = Task.Run(async () =>
        {
            while (l.IsListening)
            {
                HttpListenerContext ctx;
                try
                {
                    ctx = await l.GetContextAsync();
                }
                catch (HttpListenerException)
                {
                    return;
                }
                catch (ObjectDisposedException)
                {
                    return;
                }

                var body = page;
                if (ctx.Request.Url!.AbsolutePath == "/missing")
                {
                    ctx.Response.StatusCode = 404;
                    body = [];
                }

                await ctx.Response.OutputStream.WriteAsync(body);
                ctx.Response.Close();
            }
        });
        return (l, b);
    }

    [Fact]
    public async Task OffUnlessEnabled()
    {
        using var app = new TestApp();
        var r = await app.ClientFor("alice", "Alice").PostAsJsonAsync(Fetch, new { url = "https://example.com/" });
        Assert.Equal(HttpStatusCode.NotFound, r.StatusCode);
    }

    [Fact]
    public async Task RelaysPagesWhenEnabled()
    {
        var page = Encoding.UTF8.GetBytes("<table><tr><td>1</td></tr></table>");
        var (l, b) = Serve(page);
        using var _l = l;
        using var app = new TestApp(settings: new()
        {
            ["WebData:Enabled"] = "true",
            ["WebData:AllowPrivateNetworks"] = "true",
            ["WebData:MaxBytes"] = "1000",
        });
        var client = app.ClientFor("alice", "Alice");
        var r = await client.PostAsJsonAsync(Fetch, new { url = b + "prices" });
        Assert.Equal(HttpStatusCode.OK, r.StatusCode);
        Assert.Equal(page, await r.Content.ReadAsByteArrayAsync());
        r = await client.PostAsJsonAsync(Fetch, new { url = b + "missing" });
        Assert.Equal(HttpStatusCode.BadGateway, r.StatusCode);
        l.Stop();
    }

    [Fact]
    public async Task RefusesPrivateAddressesStrangersBadInputAndLargePages()
    {
        using var app = new TestApp(settings: new() { ["WebData:Enabled"] = "true" });
        var client = app.ClientFor("alice", "Alice");
        var r = await client.PostAsJsonAsync(Fetch, new { url = "http://127.0.0.1:9/" });
        Assert.Equal(HttpStatusCode.BadGateway, r.StatusCode);
        Assert.Contains("not allowed", await r.Content.ReadAsStringAsync(), StringComparison.Ordinal);
        foreach (var bad in new[] { "file:///etc/passwd", "ftp://example.com/", "http://user:pw@example.com/", "nope" })
        {
            Assert.Equal(HttpStatusCode.BadRequest, (await client.PostAsJsonAsync(Fetch, new { url = bad })).StatusCode);
        }

        r = await app.CreateClient().PostAsJsonAsync(Fetch, new { url = "https://example.com/" });
        Assert.Equal(HttpStatusCode.Unauthorized, r.StatusCode);

        var (l, b) = Serve(new byte[5000]);
        using var _l = l;
        using var small = new TestApp(settings: new()
        {
            ["WebData:Enabled"] = "true",
            ["WebData:AllowPrivateNetworks"] = "true",
            ["WebData:MaxBytes"] = "100",
        });
        r = await small.ClientFor("alice", "Alice").PostAsJsonAsync(Fetch, new { url = b });
        Assert.Equal(HttpStatusCode.RequestEntityTooLarge, r.StatusCode);
        l.Stop();
    }
}
