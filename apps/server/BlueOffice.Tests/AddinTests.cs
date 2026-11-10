using System.Net;
using System.Net.Http.Json;
using System.Text.Json;
using BlueOffice.Api.Addins;
using BlueOffice.Api.Sessions;
using Xunit;

namespace BlueOffice.Tests;

/// <summary>Manifest rules of the server (same as <c>packages/addin-sdk</c>, CORE-ADDIN-001).</summary>
public sealed class AddinManifestValidatorTests
{
    internal static Dictionary<string, object?> Sample(string id = "vn.example.hello") => new()
    {
        ["manifestVersion"] = 1,
        ["id"] = id,
        ["name"] = new Dictionary<string, string> { ["vi"] = "Xin chào", ["en"] = "Hello" },
        ["version"] = "1.0.0",
        ["publisher"] = "Example Co.",
        ["startUrl"] = "https://addins.example.vn/hello/index.html",
        ["allowedOrigins"] = new[] { "https://addins.example.vn" },
        ["permissions"] = new[] { "selection", "write-document", "ui:ribbon", "network:https://api.example.vn" },
        ["apps"] = new[] { "doc", "sheet" },
        ["ribbon"] = new Dictionary<string, object>
        {
            ["tab"] = "home",
            ["commands"] = new[]
            {
                new Dictionary<string, object> { ["id"] = "open", ["label"] = new Dictionary<string, string> { ["vi"] = "Mở", ["en"] = "Open" }, ["action"] = "openPane" },
            },
        },
    };

    private static List<string> Errors(Dictionary<string, object?> m, bool loopback = false)
    {
        var el = JsonSerializer.SerializeToElement(m);
        AddinManifestValidator.Validate(el, loopback, out var errors);
        return [.. errors.Select(e => e.Path)];
    }

    [Fact]
    public void AcceptsAValidManifest()
    {
        var m = AddinManifestValidator.Validate(JsonSerializer.SerializeToElement(Sample()), false, out var errors);
        Assert.Empty(errors);
        Assert.NotNull(m);
        Assert.Equal("vn.example.hello", m!.Id);
        Assert.Equal("Xin chào", m.NameVi);
        Assert.Equal(["doc", "sheet"], m.Apps);
        Assert.Equal(["https://addins.example.vn"], m.Origins);
    }

    [Fact]
    public void ReportsMissingFieldsAndUnknownProperties()
    {
        Assert.Equal(AddinManifestValidator.Required, Errors([]));
        var m = Sample();
        m["permision"] = new[] { "storage" };
        Assert.Equal(["permision"], Errors(m));
    }

    [Theory]
    [InlineData("id", "Hello")]
    [InlineData("version", "1.0")]
    [InlineData("startUrl", "https://evil.example/x")]
    [InlineData("icon", "https://user:pw@addins.example.vn/i.png")]
    [InlineData("manifestVersion", 2)]
    [InlineData("publisher", "")]
    public void RejectsBadValues(string key, object value)
    {
        var m = Sample();
        m[key] = value;
        Assert.Contains(key, Errors(m));
    }

    [Fact]
    public void ChecksOriginsPermissionsAndRibbon()
    {
        var m = Sample();
        m["allowedOrigins"] = new[] { "https://addins.example.vn/path", "http://localhost:5555" };
        m["startUrl"] = "http://localhost:5555/";
        var errors = Errors(m);
        Assert.Contains("allowedOrigins[0]", errors);
        Assert.Contains("allowedOrigins[1]", errors);
        // Development origins only when allowed.
        m["allowedOrigins"] = new[] { "http://localhost:5555" };
        Assert.Empty(Errors(m, loopback: true));

        var p = Sample();
        p["permissions"] = new object[] { "storage", "storage", "admin", 3 };
        var pe = Errors(p);
        Assert.Contains("permissions[1]", pe);
        Assert.Contains("permissions[2]", pe);
        Assert.Contains("permissions[3]", pe);
        // Ribbon commands without ui:ribbon.
        Assert.Contains("ribbon", pe);
    }
}

/// <summary>
/// Catalogue API (CORE-ADDIN-001): administrators publish, users list / install / uninstall (signed in or through
/// an editor session); everything answers 404 while <c>Addins:Enabled</c> is false.
/// </summary>
public sealed class AddinCatalogueTests : IAsyncLifetime
{
    private static readonly Dictionary<string, object> AdminClaims = new() { ["roles"] = new[] { "blueoffice-admin" } };
    private MockDrive _drive = null!;
    private TestApp _app = null!;

    public async Task InitializeAsync()
    {
        _drive = await MockDrive.StartAsync();
        _app = new TestApp(_drive, settings: new() { ["Addins:Enabled"] = "true" });
    }

    public async Task DisposeAsync()
    {
        _app.Dispose();
        await _drive.DisposeAsync();
    }

    private static Uri U(string path) => new(path, UriKind.Relative);

    private HttpClient Admin() => _app.ClientWithClaims("admin1", AdminClaims);

    private static Task<HttpResponseMessage> PublishAsync(HttpClient c, Dictionary<string, object?> manifest, bool? trusted = null) =>
        c.PutAsJsonAsync(U($"/api/admin/addins/{manifest["id"]}"), new { manifest, trusted });

    private static async Task<List<JsonElement>> ListAsync(HttpClient c, string query = "") =>
        (await c.GetFromJsonAsync<JsonElement>(U("/api/addins" + query))).GetProperty("addins").EnumerateArray().ToList();

    [Fact]
    public async Task OnlyAdministratorsPublish()
    {
        var m = AddinManifestValidatorTests.Sample();
        Assert.Equal(HttpStatusCode.Unauthorized, (await PublishAsync(_app.CreateClient(), m)).StatusCode);
        Assert.Equal(HttpStatusCode.Forbidden, (await PublishAsync(_app.ClientFor("u1"), m)).StatusCode);
        Assert.Equal(HttpStatusCode.Forbidden, (await PublishAsync(_app.ClientWithClaims("u2", new Dictionary<string, object> { ["roles"] = new[] { "user" } }), m)).StatusCode);
        Assert.Equal(HttpStatusCode.Forbidden, (await _app.ClientFor("u1").GetAsync(U("/api/admin/addins"))).StatusCode);

        var created = await PublishAsync(Admin(), m);
        Assert.Equal(HttpStatusCode.Created, created.StatusCode);
        var body = await created.Content.ReadFromJsonAsync<JsonElement>();
        Assert.Equal("1.0.0", body.GetProperty("version").GetString());
        Assert.False(body.GetProperty("trusted").GetBoolean());
        Assert.Equal("user:admin1", body.GetProperty("publishedBy").GetString());

        // Keycloak realm roles work too; an update keeps the entry (200) and may mark it trusted.
        m["version"] = "1.1.0";
        var keycloak = _app.ClientWithClaims("admin2", new Dictionary<string, object> { ["realm_access"] = new Dictionary<string, object> { ["roles"] = new[] { "blueoffice-admin" } } });
        var updated = await PublishAsync(keycloak, m, trusted: true);
        Assert.Equal(HttpStatusCode.OK, updated.StatusCode);
        var list = (await Admin().GetFromJsonAsync<JsonElement>(U("/api/admin/addins"))).GetProperty("addins").EnumerateArray().ToList();
        var only = Assert.Single(list);
        Assert.Equal("1.1.0", only.GetProperty("version").GetString());
        Assert.True(only.GetProperty("trusted").GetBoolean());
        Assert.Equal(0, only.GetProperty("installs").GetInt32());
    }

    [Fact]
    public async Task InvalidManifestsAreRejectedWithTheirErrors()
    {
        var m = AddinManifestValidatorTests.Sample();
        m["startUrl"] = "https://evil.example/";
        var res = await PublishAsync(Admin(), m);
        Assert.Equal(HttpStatusCode.BadRequest, res.StatusCode);
        var body = await res.Content.ReadFromJsonAsync<JsonElement>();
        Assert.Contains(body.GetProperty("errors").EnumerateArray(), e => e.GetProperty("path").GetString() == "startUrl");

        // The id in the URL must match.
        var other = AddinManifestValidatorTests.Sample();
        var wrong = await Admin().PutAsJsonAsync(U("/api/admin/addins/vn.example.other"), new { manifest = other });
        Assert.Equal(HttpStatusCode.BadRequest, wrong.StatusCode);
    }

    [Fact]
    public async Task ApprovedOriginsLimitWhatCanBePublished()
    {
        using var app = new TestApp(_drive, settings: new()
        {
            ["Addins:Enabled"] = "true",
            ["Addins:ApprovedOrigins:0"] = "https://addins.example.vn",
        });
        var admin = app.ClientWithClaims("admin1", AdminClaims);
        var m = AddinManifestValidatorTests.Sample();
        // network:https://api.example.vn is not approved.
        var res = await admin.PutAsJsonAsync(U($"/api/admin/addins/{m["id"]}"), new { manifest = m });
        Assert.Equal(HttpStatusCode.BadRequest, res.StatusCode);
        Assert.Contains("api.example.vn", await res.Content.ReadAsStringAsync());
        m["permissions"] = new[] { "selection" };
        m.Remove("ribbon");
        Assert.Equal(HttpStatusCode.Created, (await admin.PutAsJsonAsync(U($"/api/admin/addins/{m["id"]}"), new { manifest = m })).StatusCode);
    }

    [Fact]
    public async Task UsersListInstallAndUninstall()
    {
        await PublishAsync(Admin(), AddinManifestValidatorTests.Sample());
        var slideOnly = AddinManifestValidatorTests.Sample("vn.example.slides");
        slideOnly["apps"] = new[] { "slide" };
        slideOnly.Remove("ribbon");
        slideOnly["permissions"] = Array.Empty<string>();
        await PublishAsync(Admin(), slideOnly);

        Assert.Equal(HttpStatusCode.Unauthorized, (await _app.CreateClient().GetAsync(U("/api/addins"))).StatusCode);
        var u1 = _app.ClientFor("u1");
        var all = await ListAsync(u1);
        Assert.Equal(2, all.Count);
        var doc = Assert.Single(await ListAsync(u1, "?app=doc"));
        Assert.Equal("vn.example.hello", doc.GetProperty("manifest").GetProperty("id").GetString());
        Assert.False(doc.GetProperty("installed").GetBoolean());
        Assert.Equal(HttpStatusCode.BadRequest, (await u1.GetAsync(U("/api/addins?app=word"))).StatusCode);

        // Consent is to every permission of the manifest.
        var partial = await u1.PostAsJsonAsync(U("/api/addins/vn.example.hello/install"), new { permissions = new[] { "selection" } });
        Assert.Equal(HttpStatusCode.BadRequest, partial.StatusCode);
        var perms = new[] { "selection", "write-document", "ui:ribbon", "network:https://api.example.vn" };
        var ok = await u1.PostAsJsonAsync(U("/api/addins/vn.example.hello/install"), new { permissions = perms });
        Assert.Equal(HttpStatusCode.OK, ok.StatusCode);
        Assert.Equal(HttpStatusCode.NotFound, (await u1.PostAsJsonAsync(U("/api/addins/vn.example.nope/install"), new { permissions = perms })).StatusCode);

        doc = Assert.Single(await ListAsync(u1, "?app=doc"));
        Assert.True(doc.GetProperty("installed").GetBoolean());
        Assert.Equal(perms, doc.GetProperty("granted").EnumerateArray().Select(x => x.GetString()).ToArray());
        // Installs are per user.
        Assert.False(Assert.Single(await ListAsync(_app.ClientFor("u2"), "?app=doc")).GetProperty("installed").GetBoolean());

        var admin = (await Admin().GetFromJsonAsync<JsonElement>(U("/api/admin/addins"))).GetProperty("addins").EnumerateArray();
        Assert.Equal(1, admin.First(a => a.GetProperty("id").GetString() == "vn.example.hello").GetProperty("installs").GetInt32());

        Assert.Equal(HttpStatusCode.NoContent, (await u1.DeleteAsync(U("/api/addins/vn.example.hello/install"))).StatusCode);
        Assert.Equal(HttpStatusCode.NotFound, (await u1.DeleteAsync(U("/api/addins/vn.example.hello/install"))).StatusCode);
        Assert.False(Assert.Single(await ListAsync(u1, "?app=doc")).GetProperty("installed").GetBoolean());
    }

    [Fact]
    public async Task EditorSessionsUseTheCatalogueAsTheirUser()
    {
        await PublishAsync(Admin(), AddinManifestValidatorTests.Sample());
        var id = await _drive.CreateFileAsync("Hợp đồng.docx", "v1");
        var token = await _drive.TokenAsync("u1", id);
        var res = await _app.CreateClient().PostAsJsonAsync(U("/api/sessions"), new { host = "bluedrive", fileId = id, token });
        Assert.Equal(HttpStatusCode.Created, res.StatusCode);
        var s = await res.Content.ReadFromJsonAsync<JsonElement>();
        var sid = s.GetProperty("sessionId").GetString();
        var c = _app.CreateClient();
        // Without the session token the session is not usable.
        Assert.Equal(HttpStatusCode.Unauthorized, (await c.GetAsync(U($"/api/addins?session={sid}"))).StatusCode);
        c.DefaultRequestHeaders.Add(SessionEndpoints.TokenHeader, s.GetProperty("sessionToken").GetString());
        Assert.Single(await ListAsync(c, $"?app=doc&session={sid}"));
        var perms = new[] { "selection", "write-document", "ui:ribbon", "network:https://api.example.vn" };
        Assert.Equal(HttpStatusCode.OK, (await c.PostAsJsonAsync(U($"/api/addins/vn.example.hello/install?session={sid}"), new { permissions = perms })).StatusCode);
        Assert.True(Assert.Single(await ListAsync(c, $"?app=doc&session={sid}")).GetProperty("installed").GetBoolean());
    }

    [Fact]
    public async Task UnpublishingRemovesInstallsAndDisablingHidesIt()
    {
        var m = AddinManifestValidatorTests.Sample();
        await PublishAsync(Admin(), m);
        var u1 = _app.ClientFor("u1");
        await u1.PostAsJsonAsync(U("/api/addins/vn.example.hello/install"), new { permissions = new[] { "selection", "write-document", "ui:ribbon", "network:https://api.example.vn" } });

        var disabled = await Admin().PutAsJsonAsync(U("/api/admin/addins/vn.example.hello"), new { manifest = m, enabled = false });
        Assert.Equal(HttpStatusCode.OK, disabled.StatusCode);
        Assert.Empty(await ListAsync(u1));

        Assert.Equal(HttpStatusCode.Forbidden, (await u1.DeleteAsync(U("/api/admin/addins/vn.example.hello"))).StatusCode);
        Assert.Equal(HttpStatusCode.NoContent, (await Admin().DeleteAsync(U("/api/admin/addins/vn.example.hello"))).StatusCode);
        Assert.Equal(HttpStatusCode.NotFound, (await Admin().DeleteAsync(U("/api/admin/addins/vn.example.hello"))).StatusCode);
        // Published again: the old install is gone.
        await PublishAsync(Admin(), m);
        Assert.False(Assert.Single(await ListAsync(u1)).GetProperty("installed").GetBoolean());
    }

    [Fact]
    public async Task DisabledFeatureAnswers404Everywhere()
    {
        using var off = new TestApp(_drive);
        var admin = off.ClientWithClaims("admin1", AdminClaims);
        var user = off.ClientFor("u1");
        Assert.Equal(HttpStatusCode.NotFound, (await admin.GetAsync(U("/api/admin/addins"))).StatusCode);
        Assert.Equal(HttpStatusCode.NotFound, (await admin.PutAsJsonAsync(U("/api/admin/addins/vn.example.hello"), new { manifest = AddinManifestValidatorTests.Sample() })).StatusCode);
        Assert.Equal(HttpStatusCode.NotFound, (await admin.DeleteAsync(U("/api/admin/addins/vn.example.hello"))).StatusCode);
        Assert.Equal(HttpStatusCode.NotFound, (await user.GetAsync(U("/api/addins"))).StatusCode);
        Assert.Equal(HttpStatusCode.NotFound, (await user.PostAsJsonAsync(U("/api/addins/vn.example.hello/install"), new { permissions = Array.Empty<string>() })).StatusCode);
        Assert.Equal(HttpStatusCode.NotFound, (await user.DeleteAsync(U("/api/addins/vn.example.hello/install"))).StatusCode);
        // Anonymous callers learn nothing more.
        Assert.Equal(HttpStatusCode.NotFound, (await off.CreateClient().GetAsync(U("/api/addins"))).StatusCode);
    }
}
