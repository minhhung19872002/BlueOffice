using System.Buffers.Binary;
using System.Security.Cryptography;
using System.Text;
using System.Xml.Linq;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging;

namespace BlueOffice.Infrastructure.Hosts;

/// <summary>
/// Proof keys of BlueOffice as a WOPI client ([MS-WOPI] §2.2.1 / "Verifying that requests originate from the WOPI
/// client"): every request to a WOPI host carries <c>X-WOPI-Proof</c> (current key), <c>X-WOPI-ProofOld</c> (previous
/// key) and <c>X-WOPI-TimeStamp</c>; the public keys are published in discovery (<c>&lt;proof-key&gt;</c>).
/// Section <c>Wopi</c>: <c>ProofKey</c> / <c>OldProofKey</c> = PEM text or path of a PEM file (RSA private key).
/// </summary>
public sealed class WopiProofKeys : IDisposable
{
    private WopiProofKeys(RSA current, RSA? old, bool generated)
    {
        Current = current;
        Old = old;
        Generated = generated;
    }

    /// <summary>Key signing <c>X-WOPI-Proof</c>.</summary>
    public RSA Current { get; }

    /// <summary>Previous key signing <c>X-WOPI-ProofOld</c> after a rotation (null: the current key signs both).</summary>
    public RSA? Old { get; }

    /// <summary>The current key was generated at startup (development only; hosts cache discovery, so it must be stable).</summary>
    public bool Generated { get; }

    /// <summary>Keys from PEM text (tests, tooling).</summary>
    public static WopiProofKeys FromPem(string currentPem, string? oldPem = null) =>
        new(ImportPem(currentPem), oldPem is null ? null : ImportPem(oldPem), generated: false);

    /// <summary>
    /// Loads <c>Wopi:ProofKey</c> / <c>Wopi:OldProofKey</c>. Without a configured key a temporary one is generated, which is
    /// refused in Production when a WOPI host is configured (like <c>Auth:DevSigningKey</c>).
    /// </summary>
    public static WopiProofKeys Load(IConfiguration config, IHostEnvironment env, bool wopiHostConfigured, ILogger logger)
    {
        var current = Read(config["Wopi:ProofKey"]);
        var old = Read(config["Wopi:OldProofKey"]);
        if (current is not null)
        {
            return new WopiProofKeys(ImportPem(current), old is null ? null : ImportPem(old), generated: false);
        }

        if (wopiHostConfigured && env.IsProduction())
        {
            throw new InvalidOperationException("Wopi:ProofKey must be configured in Production when a WOPI host is configured.");
        }

        if (wopiHostConfigured)
        {
            logger.LogWarning("Wopi:ProofKey is not configured: using a temporary proof key (development only).");
        }

        return new WopiProofKeys(RSA.Create(2048), null, generated: true);
    }

    private static string? Read(string? value)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            return null;
        }

        return value.Contains("-----BEGIN", StringComparison.Ordinal) ? value : File.ReadAllText(value);
    }

    private static RSA ImportPem(string pem)
    {
        var rsa = RSA.Create();
        rsa.ImportFromPem(pem);
        return rsa;
    }

    /// <summary>Signs a request (current key for <c>X-WOPI-Proof</c>, old or current key for <c>X-WOPI-ProofOld</c>).</summary>
    public (string Proof, string ProofOld) Sign(string accessToken, string url, long timestamp)
    {
        var data = WopiProof.Payload(accessToken, url, timestamp);
        var proof = Convert.ToBase64String(Current.SignData(data, HashAlgorithmName.SHA256, RSASignaturePadding.Pkcs1));
        var proofOld = Old is null ? proof : Convert.ToBase64String(Old.SignData(data, HashAlgorithmName.SHA256, RSASignaturePadding.Pkcs1));
        return (proof, proofOld);
    }

    /// <summary>The <c>&lt;proof-key&gt;</c> element of discovery ([MS-WOPI] §3.1.5.1.1.1).</summary>
    public XElement DiscoveryElement()
    {
        var cur = WopiProof.Describe(Current);
        var old = WopiProof.Describe(Old ?? Current);
        return new XElement(
            "proof-key",
            new XAttribute("oldvalue", old.Value),
            new XAttribute("oldmodulus", old.Modulus),
            new XAttribute("oldexponent", old.Exponent),
            new XAttribute("value", cur.Value),
            new XAttribute("modulus", cur.Modulus),
            new XAttribute("exponent", cur.Exponent));
    }

    /// <inheritdoc />
    public void Dispose()
    {
        Current.Dispose();
        Old?.Dispose();
    }
}

/// <summary>Public form of a proof key: CSP public key blob (<c>value</c>), modulus and exponent (Base64, big-endian).</summary>
public sealed record WopiProofKeyInfo(string Value, string Modulus, string Exponent);

/// <summary>Proof data and key encodings of [MS-WOPI] proof keys.</summary>
public static class WopiProof
{
    /// <summary>Request header with the proof made with the current key.</summary>
    public const string ProofHeader = "X-WOPI-Proof";

    /// <summary>Request header with the proof made with the previous key.</summary>
    public const string ProofOldHeader = "X-WOPI-ProofOld";

    /// <summary>Request header with the timestamp (UTC ticks: 100 ns since 0001-01-01).</summary>
    public const string TimeStampHeader = "X-WOPI-TimeStamp";

    /// <summary>Maximum age of a proof timestamp.</summary>
    public static readonly TimeSpan MaxAge = TimeSpan.FromMinutes(20);

    /// <summary>
    /// Signed bytes: 4-byte big-endian length + UTF-8 access token, 4-byte length + upper-cased absolute URL (with the query
    /// string), 4-byte length (8) + 8-byte big-endian timestamp.
    /// </summary>
    public static byte[] Payload(string accessToken, string url, long timestamp)
    {
        var token = Encoding.UTF8.GetBytes(accessToken);
        var u = Encoding.UTF8.GetBytes(url.ToUpperInvariant());
        var buffer = new byte[4 + token.Length + 4 + u.Length + 4 + 8];
        var span = buffer.AsSpan();
        BinaryPrimitives.WriteInt32BigEndian(span, token.Length);
        token.CopyTo(span[4..]);
        var at = 4 + token.Length;
        BinaryPrimitives.WriteInt32BigEndian(span[at..], u.Length);
        u.CopyTo(span[(at + 4)..]);
        at += 4 + u.Length;
        BinaryPrimitives.WriteInt32BigEndian(span[at..], 8);
        BinaryPrimitives.WriteInt64BigEndian(span[(at + 4)..], timestamp);
        return buffer;
    }

    /// <summary>Public key of <paramref name="rsa"/> in the encodings used by discovery.</summary>
    public static WopiProofKeyInfo Describe(RSA rsa)
    {
        var p = rsa.ExportParameters(includePrivateParameters: false);
        var modulus = p.Modulus ?? throw new CryptographicException("RSA key without modulus.");
        var exponent = p.Exponent ?? throw new CryptographicException("RSA key without exponent.");
        return new WopiProofKeyInfo(Convert.ToBase64String(PublicKeyBlob(modulus, exponent)), Convert.ToBase64String(modulus), Convert.ToBase64String(exponent));
    }

    /// <summary>
    /// CryptoAPI <c>PUBLICKEYBLOB</c> (little-endian): BLOBHEADER {bType 0x06, bVersion 0x02, reserved 0, aiKeyAlg
    /// CALG_RSA_KEYX 0x0000A400}, RSAPUBKEY {magic "RSA1", bitlen, pubexp}, modulus (little-endian).
    /// </summary>
    public static byte[] PublicKeyBlob(byte[] modulus, byte[] exponent)
    {
        var blob = new byte[8 + 12 + modulus.Length];
        blob[0] = 0x06;
        blob[1] = 0x02;
        BinaryPrimitives.WriteUInt32LittleEndian(blob.AsSpan(4), 0x0000A400);
        BinaryPrimitives.WriteUInt32LittleEndian(blob.AsSpan(8), 0x31415352);
        BinaryPrimitives.WriteUInt32LittleEndian(blob.AsSpan(12), (uint)(modulus.Length * 8));
        uint e = 0;
        foreach (var b in exponent)
        {
            e = (e << 8) | b;
        }

        BinaryPrimitives.WriteUInt32LittleEndian(blob.AsSpan(16), e);
        for (var i = 0; i < modulus.Length; i++)
        {
            blob[20 + i] = modulus[modulus.Length - 1 - i];
        }

        return blob;
    }
}

/// <summary>
/// Proof validation as a WOPI host does it ([MS-WOPI] "Verifying that requests originate from the WOPI client"): the
/// request is genuine when <c>X-WOPI-Proof</c> verifies with the current key, or <c>X-WOPI-ProofOld</c> with the current
/// key, or <c>X-WOPI-Proof</c> with the old key; and the timestamp is at most 20 minutes old.
/// Used by the test WOPI host; usable by any host written in .NET.
/// </summary>
public sealed class WopiProofValidator(RSA current, RSA? old)
{
    /// <summary>Public keys from a discovery document (<c>&lt;proof-key modulus exponent oldmodulus oldexponent&gt;</c>).</summary>
    public static WopiProofValidator FromDiscovery(XDocument discovery)
    {
        var e = discovery.Descendants("proof-key").FirstOrDefault() ?? throw new InvalidDataException("Discovery has no proof-key.");
        static RSA Key(string? modulus, string? exponent)
        {
            var rsa = RSA.Create();
            rsa.ImportParameters(new RSAParameters { Modulus = Convert.FromBase64String(modulus ?? ""), Exponent = Convert.FromBase64String(exponent ?? "") });
            return rsa;
        }

        var cur = Key((string?)e.Attribute("modulus"), (string?)e.Attribute("exponent"));
        var oldModulus = (string?)e.Attribute("oldmodulus");
        return new WopiProofValidator(cur, string.IsNullOrEmpty(oldModulus) ? null : Key(oldModulus, (string?)e.Attribute("oldexponent")));
    }

    /// <summary>True when the proof headers of a request to <paramref name="url"/> are valid at <paramref name="now"/>.</summary>
    public bool IsValid(string accessToken, string url, string? proof, string? proofOld, string? timestamp, DateTimeOffset now)
    {
        if (!long.TryParse(timestamp, System.Globalization.NumberStyles.None, System.Globalization.CultureInfo.InvariantCulture, out var ticks)
            || ticks < DateTime.MinValue.Ticks || ticks > DateTime.MaxValue.Ticks)
        {
            return false;
        }

        var age = now.UtcDateTime - new DateTime(ticks, DateTimeKind.Utc);
        if (age > WopiProof.MaxAge || age < -WopiProof.MaxAge)
        {
            return false;
        }

        var data = WopiProof.Payload(accessToken, url, ticks);
        return Verify(current, data, proof) || Verify(current, data, proofOld) || (old is not null && Verify(old, data, proof));
    }

    private static bool Verify(RSA key, byte[] data, string? signature)
    {
        if (string.IsNullOrEmpty(signature))
        {
            return false;
        }

        try
        {
            return key.VerifyData(data, Convert.FromBase64String(signature), HashAlgorithmName.SHA256, RSASignaturePadding.Pkcs1);
        }
        catch (FormatException)
        {
            return false;
        }
    }
}
