using System.Net;
using System.Net.Http.Json;
using BlueOffice.Api.Signing;
using Xunit;

namespace BlueOffice.Tests;

/// <summary>The CRL / OCSP proxy used by the browser verifier (ADR-0007).</summary>
public sealed class RevocationProxyTests
{
    private static readonly Uri Fetch = new("/api/revocation/fetch", UriKind.Relative);

    [Theory]
    [InlineData("8.8.8.8", true)]
    [InlineData("2001:4860:4860::8888", true)]
    [InlineData("127.0.0.1", false)]
    [InlineData("10.1.2.3", false)]
    [InlineData("172.20.0.1", false)]
    [InlineData("192.168.1.1", false)]
    [InlineData("169.254.169.254", false)]
    [InlineData("100.64.0.1", false)]
    [InlineData("0.0.0.0", false)]
    [InlineData("::1", false)]
    [InlineData("fe80::1", false)]
    [InlineData("fd00::1", false)]
    [InlineData("::ffff:127.0.0.1", false)]
    public void PublicAddresses(string ip, bool expected) =>
        Assert.Equal(expected, RevocationProxy.IsPublic(IPAddress.Parse(ip)));

    /// <summary>A local HTTP server answering CRL GETs and OCSP POSTs.</summary>
    private static (HttpListener Listener, string Base, Task Loop) Serve(byte[] crl, byte[] ocsp)
    {
        var port = Random.Shared.Next(20000, 60000);
        var l = new HttpListener();
        var b = $"http://127.0.0.1:{port}/";
        l.Prefixes.Add(b);
        l.Start();
        var loop = Task.Run(async () =>
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

                var body = ctx.Request.HttpMethod == "POST" && ctx.Request.ContentType == "application/ocsp-request" ? ocsp : crl;
                if (ctx.Request.Url!.AbsolutePath == "/missing")
                {
                    ctx.Response.StatusCode = 404;
                    body = [];
                }

                await ctx.Response.OutputStream.WriteAsync(body);
                ctx.Response.Close();
            }
        });
        return (l, b, loop);
    }

    [Fact]
    public async Task RelaysCrlsAndOcspResponses()
    {
        byte[] crl = [0x30, 0x03, 1, 2, 3];
        byte[] ocsp = [0x30, 0x02, 9, 9];
        var (l, b, _) = Serve(crl, ocsp);
        using var _l = l;
        using var app = new TestApp(settings: new() { ["Revocation:AllowPrivateNetworks"] = "true" });
        var client = app.ClientFor("alice", "Alice");
        var r = await client.PostAsJsonAsync(Fetch, new { kind = "crl", url = b + "ca.crl" });
        Assert.Equal(HttpStatusCode.OK, r.StatusCode);
        Assert.Equal(crl, await r.Content.ReadAsByteArrayAsync());
        r = await client.PostAsJsonAsync(Fetch, new { kind = "ocsp", url = b, body = Convert.ToBase64String([0x30, 0x00]) });
        Assert.Equal(ocsp, await r.Content.ReadAsByteArrayAsync());
        r = await client.PostAsJsonAsync(Fetch, new { kind = "crl", url = b + "missing" });
        Assert.Equal(HttpStatusCode.BadGateway, r.StatusCode);
        l.Stop();
    }

    [Fact]
    public async Task RefusesPrivateAddressesStrangersAndBadInput()
    {
        using var app = new TestApp();
        var client = app.ClientFor("alice", "Alice");
        // Loopback (and private networks) are not reachable by default.
        var r = await client.PostAsJsonAsync(Fetch, new { kind = "crl", url = "http://127.0.0.1:9/ca.crl" });
        Assert.Equal(HttpStatusCode.BadGateway, r.StatusCode);
        Assert.Contains("not allowed", await r.Content.ReadAsStringAsync(), StringComparison.Ordinal);
        foreach (var bad in new object[]
        {
            new { kind = "crl", url = "file:///etc/passwd" },
            new { kind = "crl", url = "ftp://example.com/ca.crl" },
            new { kind = "crl", url = "http://user:pw@example.com/ca.crl" },
            new { kind = "x", url = "http://example.com/" },
            new { kind = "ocsp", url = "http://example.com/", body = "%%%" },
        })
        {
            Assert.Equal(HttpStatusCode.BadRequest, (await client.PostAsJsonAsync(Fetch, bad)).StatusCode);
        }

        // Neither signed in nor in an editor session.
        r = await app.CreateClient().PostAsJsonAsync(Fetch, new { kind = "crl", url = "http://example.com/ca.crl" });
        Assert.Equal(HttpStatusCode.Unauthorized, r.StatusCode);
    }
}
