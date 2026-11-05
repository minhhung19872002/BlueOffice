namespace BlueOffice.Domain;

/// <summary>Target formats supported by the conversion worker (bo-cli).</summary>
public enum ConversionTarget
{
    Pdf,
}

/// <summary>Result of a conversion.</summary>
public sealed record ConversionResult(byte[] Content, string ContentType, string FileName);

/// <summary>Converts documents using the Rust core (headless). Implemented in Infrastructure.</summary>
public interface IDocumentConverter
{
    Task<ConversionResult> ConvertAsync(Stream input, string fileName, ConversionTarget target, CancellationToken ct);
}

/// <summary>Thrown when the input cannot be converted (bad file, unsupported format).</summary>
public sealed class ConversionException(string message) : Exception(message);
