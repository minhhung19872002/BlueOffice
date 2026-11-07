namespace BlueOffice.Domain;

/// <summary>Binary object storage (MinIO / S3 in production, file system for dev and tests).</summary>
public interface IObjectStore
{
    /// <summary>Stores the stream under <paramref name="key"/> (overwrites). Returns the number of bytes written.</summary>
    Task<long> PutAsync(string key, Stream content, string contentType, CancellationToken ct);

    /// <summary>Opens the object for reading. Throws <see cref="ObjectNotFoundException"/>.</summary>
    Task<Stream> OpenReadAsync(string key, CancellationToken ct);

    /// <summary>True if the object exists.</summary>
    Task<bool> ExistsAsync(string key, CancellationToken ct);

    /// <summary>Deletes the object (no error if missing).</summary>
    Task DeleteAsync(string key, CancellationToken ct);

    /// <summary>Copies an object.</summary>
    Task CopyAsync(string sourceKey, string destinationKey, CancellationToken ct);

    /// <summary>Lists keys starting with <paramref name="prefix"/>, ordered ordinally.</summary>
    Task<IReadOnlyList<string>> ListAsync(string prefix, CancellationToken ct);
}

/// <summary>Object not found in the store.</summary>
public sealed class ObjectNotFoundException(string key) : Exception($"Object '{key}' not found.");

/// <summary>Upload larger than the configured limit.</summary>
public sealed class PayloadTooLargeException(long limit) : Exception($"Payload exceeds {limit} bytes.")
{
    /// <summary>Limit in bytes.</summary>
    public long Limit { get; } = limit;
}

/// <summary>Helpers for object keys.</summary>
public static class ObjectKeys
{
    /// <summary>Validates a key: lower-case ASCII letters, digits, '/', '-', '_', '.'; no "..", no leading '/'.</summary>
    public static bool IsValid(string key) =>
        key.Length is > 0 and <= 512
        && !key.StartsWith('/')
        && !key.Contains("..", StringComparison.Ordinal)
        && key.All(c => c is (>= 'a' and <= 'z') or (>= '0' and <= '9') or '/' or '-' or '_' or '.');

    /// <summary>Throws if the key is invalid.</summary>
    public static string Check(string key) =>
        IsValid(key) ? key : throw new ArgumentException($"Invalid object key '{key}'.", nameof(key));
}
