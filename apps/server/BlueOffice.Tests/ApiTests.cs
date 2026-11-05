using System.Net;
using System.Net.Http.Headers;
using System.Net.Http.Json;
using BlueOffice.Domain;
using Microsoft.AspNetCore.Mvc.Testing;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.DependencyInjection.Extensions;
using Xunit;

namespace BlueOffice.Tests;

public sealed class ApiTests(WebApplicationFactory<Program> factory) : IClassFixture<WebApplicationFactory<Program>>
{
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

    private HttpClient Client() => factory.WithWebHostBuilder(b => b.ConfigureServices(s =>
    {
        s.RemoveAll<IDocumentConverter>();
        s.AddSingleton<IDocumentConverter, FakeConverter>();
    })).CreateClient();

    [Fact]
    public async Task HealthIsOk()
    {
        var res = await Client().GetAsync(new Uri("/health", UriKind.Relative));
        Assert.Equal(HttpStatusCode.OK, res.StatusCode);
    }

    [Fact]
    public async Task VersionReportsPhase()
    {
        var v = await Client().GetFromJsonAsync<VersionDto>(new Uri("/api/version", UriKind.Relative));
        Assert.NotNull(v);
        Assert.Equal("BlueOffice", v.Name);
        Assert.Equal(1, v.Phase);
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
    public async Task ConvertRejectsUnsupportedInput()
    {
        using var form = new MultipartFormDataContent();
        form.Add(new ByteArrayContent([1]), "file", "virus.exe");
        var res = await Client().PostAsync(new Uri("/api/convert/pdf", UriKind.Relative), form);
        Assert.Equal(HttpStatusCode.UnprocessableEntity, res.StatusCode);
    }

    private sealed record VersionDto(string Name, string Version, int Phase);
}
