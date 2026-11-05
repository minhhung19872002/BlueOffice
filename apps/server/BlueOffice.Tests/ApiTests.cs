using System.Net;
using System.Net.Http.Headers;
using System.Net.Http.Json;
using BlueOffice.Domain;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.DependencyInjection.Extensions;
using Xunit;

namespace BlueOffice.Tests;

public sealed class ApiTests : IDisposable
{
    private readonly TestApp _app = new();

    private sealed class FakeConverter : IDocumentConverter
    {
        public Task<ConversionResult> ConvertAsync(Stream input, string fileName, ConversionTarget target, CancellationToken ct)
        {
            if (fileName.EndsWith(".exe", StringComparison.OrdinalIgnoreCase))
            {
                throw new ConversionException("Unsupported input type '.exe'.");
            }

            return Task.FromResult(new ConversionResult("%PDF-1.7"u8.ToArray(), "application/pdf", "out.pdf"));
        }
    }

    public void Dispose() => _app.Dispose();

    private HttpClient Client(bool authenticated = true)
    {
        var c = _app.WithWebHostBuilder(b => b.ConfigureServices(s =>
        {
            s.RemoveAll<IDocumentConverter>();
            s.AddSingleton<IDocumentConverter, FakeConverter>();
        })).CreateClient();
        if (authenticated)
        {
            c.DefaultRequestHeaders.Authorization = new AuthenticationHeaderValue("Bearer", TestApp.UserToken("alice", "Alice"));
        }

        return c;
    }

    [Fact]
    public async Task HealthIsOk()
    {
        var c = Client(authenticated: false);
        Assert.Equal(HttpStatusCode.OK, (await c.GetAsync(new Uri("/health", UriKind.Relative))).StatusCode);
        Assert.Equal(HttpStatusCode.OK, (await c.GetAsync(new Uri("/health/ready", UriKind.Relative))).StatusCode);
    }

    [Fact]
    public async Task VersionReportsPhase()
    {
        var v = await Client(false).GetFromJsonAsync<VersionDto>(new Uri("/api/version", UriKind.Relative));
        Assert.NotNull(v);
        Assert.Equal("BlueOffice", v.Name);
        Assert.Equal(6, v.Phase);
    }

    [Fact]
    public async Task ConvertReturnsPdf()
    {
        using var form = new MultipartFormDataContent();
        var file = new ByteArrayContent([1, 2, 3]);
        file.Headers.ContentType = new MediaTypeHeaderValue("application/octet-stream");
        form.Add(file, "file", "a.docx");
        var res = await Client().PostAsync(new Uri("/api/convert/pdf", UriKind.Relative), form);
        Assert.Equal(HttpStatusCode.OK, res.StatusCode);
        Assert.Equal("application/pdf", res.Content.Headers.ContentType?.MediaType);
        Assert.StartsWith("%PDF", await res.Content.ReadAsStringAsync());
    }

    [Fact]
    public async Task ConvertRequiresAuthentication()
    {
        using var form = new MultipartFormDataContent();
        form.Add(new ByteArrayContent([1]), "file", "a.docx");
        var res = await Client(authenticated: false).PostAsync(new Uri("/api/convert/pdf", UriKind.Relative), form);
        Assert.Equal(HttpStatusCode.Unauthorized, res.StatusCode);
    }

    [Fact]
    public async Task ConvertRejectsUnsupportedInput()
    {
        using var form = new MultipartFormDataContent();
        form.Add(new ByteArrayContent([1]), "file", "virus.exe");
        var res = await Client().PostAsync(new Uri("/api/convert/pdf", UriKind.Relative), form);
        Assert.Equal(HttpStatusCode.UnprocessableEntity, res.StatusCode);
    }

    [Fact]
    public async Task SecurityHeadersAreSet()
    {
        var res = await Client(false).GetAsync(new Uri("/api/version", UriKind.Relative));
        Assert.Equal("nosniff", res.Headers.GetValues("X-Content-Type-Options").Single());
        Assert.Contains("https://drive.mock", res.Headers.GetValues("Content-Security-Policy").Single(), StringComparison.Ordinal);
    }

    private sealed record VersionDto(string Name, string Version, int Phase);
}
