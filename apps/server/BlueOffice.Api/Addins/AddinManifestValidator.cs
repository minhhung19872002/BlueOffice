using System.Text.Json;
using System.Text.RegularExpressions;

namespace BlueOffice.Api.Addins;

/// <summary>
/// Server copy of the add-in manifest rules (CORE-ADDIN-001; <c>packages/addin-sdk/src/manifest.ts</c> and
/// <c>manifest.schema.json</c> are the reference). Returns every error with its JSON path, like the client.
/// </summary>
public static partial class AddinManifestValidator
{
    /// <summary>Permissions without an argument.</summary>
    public static readonly string[] BasePermissions = ["read-document", "write-document", "selection", "ui:ribbon", "storage"];

    /// <summary>Applications.</summary>
    public static readonly string[] Apps = ["doc", "sheet", "slide"];

    /// <summary>Prefix of network permissions.</summary>
    public const string NetworkPrefix = "network:";

    private static readonly HashSet<string> TopKeys =
    [
        "$schema", "manifestVersion", "id", "name", "description", "version", "publisher", "icon", "startUrl",
        "allowedOrigins", "permissions", "apps", "ribbon",
    ];

    /// <summary>Required top-level fields (same order as the client).</summary>
    public static readonly string[] Required =
        ["manifestVersion", "id", "name", "version", "publisher", "startUrl", "allowedOrigins", "permissions", "apps"];

    [GeneratedRegex(@"^[a-z0-9](?:[a-z0-9-]{0,62})(?:\.[a-z0-9](?:[a-z0-9-]{0,62}))+$")]
    private static partial Regex IdRegex();

    [GeneratedRegex(@"^[A-Za-z][A-Za-z0-9_-]{0,63}$")]
    private static partial Regex CommandIdRegex();

    [GeneratedRegex(@"^(0|[1-9]\d{0,8})\.(0|[1-9]\d{0,8})\.(0|[1-9]\d{0,8})(?:-[0-9A-Za-z.-]{1,40})?$")]
    private static partial Regex VersionRegex();

    [GeneratedRegex(@"^[A-Za-z][A-Za-z0-9]{0,39}$")]
    private static partial Regex TabRegex();

    /// <summary>A validated manifest: the fields the catalogue keeps besides the JSON.</summary>
    public sealed record Manifest(string Id, string Version, string NameVi, IReadOnlyList<string> Apps, IReadOnlyList<string> Permissions, IReadOnlyList<string> Origins);

    /// <summary>
    /// <paramref name="o"/> is a canonical origin: <c>https://host[:port]</c>, or (when <paramref name="loopback"/>)
    /// <c>http://localhost[:port]</c> / <c>http://127.0.0.1[:port]</c>.
    /// </summary>
    public static bool IsOrigin(string? o, bool loopback)
    {
        if (o is null || o.Length > 300 || !Uri.TryCreate(o, UriKind.Absolute, out var u))
        {
            return false;
        }

        var canonical = u.IsDefaultPort ? $"{u.Scheme}://{u.Host}" : $"{u.Scheme}://{u.Host}:{u.Port}";
        if (!string.Equals(canonical, o, StringComparison.Ordinal) || u.HostNameType == UriHostNameType.IPv6)
        {
            return false;
        }

        return u.Scheme == Uri.UriSchemeHttps || (loopback && u.Scheme == Uri.UriSchemeHttp && (u.Host == "localhost" || u.Host == "127.0.0.1"));
    }

    private static string? OriginOf(string url)
    {
        if (url.Length > 2048 || !Uri.TryCreate(url, UriKind.Absolute, out var u) || !string.IsNullOrEmpty(u.UserInfo))
        {
            return null;
        }

        if (u.Scheme != Uri.UriSchemeHttps && u.Scheme != Uri.UriSchemeHttp)
        {
            return null;
        }

        return u.IsDefaultPort ? $"{u.Scheme}://{u.Host}" : $"{u.Scheme}://{u.Host}:{u.Port}";
    }

    /// <summary>Validates <paramref name="m"/>; <paramref name="errors"/> lists the problems (path, message).</summary>
    public static Manifest? Validate(JsonElement m, bool allowLoopback, out List<(string Path, string Message)> errors)
    {
        var errs = new List<(string, string)>();
        errors = errs;
        void Err(string path, string message) => errs.Add((path, message));
        if (m.ValueKind != JsonValueKind.Object)
        {
            Err("", "expected an object");
            return null;
        }

        foreach (var p in m.EnumerateObject())
        {
            if (!TopKeys.Contains(p.Name))
            {
                Err(p.Name, "unknown property");
            }
        }

        foreach (var k in Required)
        {
            if (!m.TryGetProperty(k, out _))
            {
                Err(k, "required");
            }
        }

        bool Text(JsonElement? v, string path, int max)
        {
            if (v is not { ValueKind: JsonValueKind.String } s || string.IsNullOrWhiteSpace(s.GetString()))
            {
                Err(path, "expected a non-empty string");
                return false;
            }

            var t = s.GetString()!;
            if (t.Length > max)
            {
                Err(path, $"at most {max} characters");
            }
            else if (t.Any(c => c < 0x20 || c == 0x7f))
            {
                Err(path, "control characters are not allowed");
            }

            return true;
        }

        void Localized(JsonElement v, string path, int max)
        {
            if (v.ValueKind != JsonValueKind.Object)
            {
                Err(path, "expected { \"vi\": \"…\", \"en\": \"…\" }");
                return;
            }

            foreach (var p in v.EnumerateObject())
            {
                if (p.Name is not "vi" and not "en")
                {
                    Err($"{path}.{p.Name}", "unknown language");
                }
            }

            Text(v.TryGetProperty("vi", out var vi) ? vi : null, $"{path}.vi", max);
            Text(v.TryGetProperty("en", out var en) ? en : null, $"{path}.en", max);
        }

        List<string> AppList(JsonElement v, string path)
        {
            var list = new List<string>();
            if (v.ValueKind != JsonValueKind.Array || v.GetArrayLength() == 0)
            {
                Err(path, "expected a non-empty list of doc, sheet, slide");
                return list;
            }

            var i = 0;
            foreach (var a in v.EnumerateArray())
            {
                var s = a.ValueKind == JsonValueKind.String ? a.GetString() : null;
                if (s is null || !Apps.Contains(s))
                {
                    Err($"{path}[{i}]", "expected doc, sheet or slide");
                }
                else if (list.Contains(s))
                {
                    Err($"{path}[{i}]", "duplicate");
                }
                else
                {
                    list.Add(s);
                }

                i++;
            }

            return list;
        }

        if (m.TryGetProperty("$schema", out var schema) && schema.ValueKind != JsonValueKind.String)
        {
            Err("$schema", "expected a string");
        }

        if (m.TryGetProperty("manifestVersion", out var mv) && !(mv.ValueKind == JsonValueKind.Number && mv.TryGetInt32(out var mvi) && mvi == 1))
        {
            Err("manifestVersion", "expected 1");
        }

        var id = "";
        if (m.TryGetProperty("id", out var idEl))
        {
            id = idEl.ValueKind == JsonValueKind.String ? idEl.GetString()! : "";
            if (id.Length is 0 or > 128 || !IdRegex().IsMatch(id))
            {
                Err("id", "expected a lower-case reverse domain name such as vn.example.hello");
            }
        }

        var nameVi = "";
        if (m.TryGetProperty("name", out var name))
        {
            Localized(name, "name", 100);
            if (name.ValueKind == JsonValueKind.Object && name.TryGetProperty("vi", out var nv) && nv.ValueKind == JsonValueKind.String)
            {
                nameVi = nv.GetString()!;
            }
        }

        if (m.TryGetProperty("description", out var desc))
        {
            Localized(desc, "description", 1000);
        }

        var version = "";
        if (m.TryGetProperty("version", out var ver))
        {
            version = ver.ValueKind == JsonValueKind.String ? ver.GetString()! : "";
            if (!VersionRegex().IsMatch(version))
            {
                Err("version", "expected a semantic version such as 1.0.0");
            }
        }

        if (m.TryGetProperty("publisher", out var pub))
        {
            Text(pub, "publisher", 100);
        }

        var origins = new List<string>();
        if (m.TryGetProperty("allowedOrigins", out var ao))
        {
            if (ao.ValueKind != JsonValueKind.Array || ao.GetArrayLength() == 0)
            {
                Err("allowedOrigins", "expected a non-empty list");
            }
            else if (ao.GetArrayLength() > 20)
            {
                Err("allowedOrigins", "at most 20 origins");
            }
            else
            {
                var i = 0;
                foreach (var o in ao.EnumerateArray())
                {
                    var s = o.ValueKind == JsonValueKind.String ? o.GetString() : null;
                    if (!IsOrigin(s, allowLoopback))
                    {
                        Err($"allowedOrigins[{i}]", "expected an origin https://host[:port] (no path)");
                    }
                    else if (origins.Contains(s!))
                    {
                        Err($"allowedOrigins[{i}]", "duplicate");
                    }
                    else
                    {
                        origins.Add(s!);
                    }

                    i++;
                }
            }
        }

        foreach (var key in new[] { "startUrl", "icon" })
        {
            if (m.TryGetProperty(key, out var url))
            {
                var o = url.ValueKind == JsonValueKind.String ? OriginOf(url.GetString()!) : null;
                if (o is null || !origins.Contains(o))
                {
                    Err(key, "expected an http(s) URL on one of allowedOrigins");
                }
            }
        }

        var perms = new List<string>();
        if (m.TryGetProperty("permissions", out var pl))
        {
            if (pl.ValueKind != JsonValueKind.Array)
            {
                Err("permissions", "expected a list");
            }
            else if (pl.GetArrayLength() > 30)
            {
                Err("permissions", "at most 30 permissions");
            }
            else
            {
                var i = 0;
                foreach (var p in pl.EnumerateArray())
                {
                    var path = $"permissions[{i++}]";
                    var s = p.ValueKind == JsonValueKind.String ? p.GetString()! : null;
                    if (s is null)
                    {
                        Err(path, "expected a string");
                    }
                    else if (perms.Contains(s))
                    {
                        Err(path, "duplicate");
                    }
                    else if (s.StartsWith(NetworkPrefix, StringComparison.Ordinal))
                    {
                        if (!IsOrigin(s[NetworkPrefix.Length..], allowLoopback))
                        {
                            Err(path, "expected network:https://host[:port]");
                        }
                        else
                        {
                            perms.Add(s);
                        }
                    }
                    else if (!BasePermissions.Contains(s))
                    {
                        Err(path, "unknown permission");
                    }
                    else
                    {
                        perms.Add(s);
                    }
                }
            }
        }

        var apps = new List<string>();
        if (m.TryGetProperty("apps", out var appsEl))
        {
            apps = AppList(appsEl, "apps");
        }

        if (m.TryGetProperty("ribbon", out var r))
        {
            if (r.ValueKind != JsonValueKind.Object)
            {
                Err("ribbon", "expected an object");
            }
            else
            {
                foreach (var p in r.EnumerateObject())
                {
                    if (p.Name is not "tab" and not "group" and not "commands")
                    {
                        Err($"ribbon.{p.Name}", "unknown property");
                    }
                }

                if (!perms.Contains("ui:ribbon"))
                {
                    Err("ribbon", "ribbon commands need the ui:ribbon permission");
                }

                if (r.TryGetProperty("tab", out var tab) && !(tab.ValueKind == JsonValueKind.String && TabRegex().IsMatch(tab.GetString()!)))
                {
                    Err("ribbon.tab", "expected a tab id such as home or insert");
                }

                if (r.TryGetProperty("group", out var group))
                {
                    Localized(group, "ribbon.group", 60);
                }

                if (!r.TryGetProperty("commands", out var cmds) || cmds.ValueKind != JsonValueKind.Array || cmds.GetArrayLength() == 0)
                {
                    Err("ribbon.commands", "expected a non-empty list");
                }
                else if (cmds.GetArrayLength() > 20)
                {
                    Err("ribbon.commands", "at most 20 commands");
                }
                else
                {
                    var ids = new List<string>();
                    var i = 0;
                    foreach (var c in cmds.EnumerateArray())
                    {
                        var path = $"ribbon.commands[{i++}]";
                        if (c.ValueKind != JsonValueKind.Object)
                        {
                            Err(path, "expected an object");
                            continue;
                        }

                        foreach (var p in c.EnumerateObject())
                        {
                            if (p.Name is not "id" and not "label" and not "tooltip" and not "action" and not "apps")
                            {
                                Err($"{path}.{p.Name}", "unknown property");
                            }
                        }

                        var cid = c.TryGetProperty("id", out var ce) && ce.ValueKind == JsonValueKind.String ? ce.GetString()! : null;
                        if (cid is null || !CommandIdRegex().IsMatch(cid))
                        {
                            Err($"{path}.id", "expected letters, digits, - or _ (starting with a letter)");
                        }
                        else if (ids.Contains(cid))
                        {
                            Err($"{path}.id", "duplicate");
                        }
                        else
                        {
                            ids.Add(cid);
                        }

                        if (c.TryGetProperty("label", out var label))
                        {
                            Localized(label, $"{path}.label", 60);
                        }
                        else
                        {
                            Err($"{path}.label", "expected { \"vi\": \"…\", \"en\": \"…\" }");
                        }

                        if (c.TryGetProperty("tooltip", out var tip))
                        {
                            Localized(tip, $"{path}.tooltip", 200);
                        }

                        var action = c.TryGetProperty("action", out var ae) && ae.ValueKind == JsonValueKind.String ? ae.GetString() : null;
                        if (action is not "openPane" and not "event")
                        {
                            Err($"{path}.action", "expected openPane or event");
                        }

                        if (c.TryGetProperty("apps", out var capps))
                        {
                            foreach (var a in AppList(capps, $"{path}.apps"))
                            {
                                if (apps.Count > 0 && !apps.Contains(a))
                                {
                                    Err($"{path}.apps", $"{a} is not one of the manifest's apps");
                                }
                            }
                        }
                    }
                }
            }
        }

        return errs.Count == 0 ? new Manifest(id, version, nameVi, apps, perms, origins) : null;
    }
}
