using BlueOffice.Domain;

namespace BlueOffice.Infrastructure.Storage;

/// <summary>Options of the object store (section <c>Storage</c>).</summary>
public sealed class StorageOptions
{
    /// <summary>"FileSystem" (dev/tests) or "Minio" (production).</summary>
    public string Provider { get; set; } = "FileSystem";

    /// <summary>Root directory of the file-system store.</summary>
    public string Root { get; set; } = "data/objects";

    /// <summary>MinIO endpoint "host:port".</summary>
    public string Endpoint { get; set; } = "localhost:9000";

    /// <summary>MinIO access key.</summary>
    public string AccessKey { get; set; } = "";

    /// <summary>MinIO secret key.</summary>
    public string SecretKey { get; set; } = "";

    /// <summary>Bucket.</summary>
    public string Bucket { get; set; } = "blueoffice";

    /// <summary>Use TLS to reach MinIO.</summary>
    public bool UseSsl { get; set; }

    /// <summary>Maximum upload size in bytes.</summary>
    public long MaxUploadBytes { get; set; } = 200L * 1024 * 1024;
}

/// <summary>Stores objects as files below a root directory (atomic replace through a temp file).</summary>
public sealed class FileSystemObjectStore : IObjectStore
{
    private const string TempDir = ".tmp";
    private readonly string _root;

    /// <summary>Creates the store rooted at <paramref name="root"/>.</summary>
    public FileSystemObjectStore(string root)
    {
        _root = Path.GetFullPath(root);
        Directory.CreateDirectory(Path.Combine(_root, TempDir));
    }

    private string PathOf(string key) => Path.Combine(_root, ObjectKeys.Check(key).Replace('/', Path.DirectorySeparatorChar));

    /// <inheritdoc />
    public async Task<long> PutAsync(string key, Stream content, string contentType, CancellationToken ct)
    {
        var path = PathOf(key);
        var tmp = Path.Combine(_root, TempDir, Guid.NewGuid().ToString("N"));
        long n;
        try
        {
            await using var fs = new FileStream(tmp, FileMode.CreateNew, FileAccess.Write, FileShare.None, 81920, useAsync: true);
            await content.CopyToAsync(fs, ct);
            n = fs.Length;
        }
        catch
        {
            File.Delete(tmp);
            throw;
        }

        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        File.Move(tmp, path, overwrite: true);
        return n;
    }

    /// <inheritdoc />
    public Task<Stream> OpenReadAsync(string key, CancellationToken ct)
    {
        var path = PathOf(key);
        if (!File.Exists(path))
        {
            throw new ObjectNotFoundException(key);
        }

        Stream s = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.Read | FileShare.Delete, 81920, useAsync: true);
        return Task.FromResult(s);
    }

    /// <inheritdoc />
    public Task<bool> ExistsAsync(string key, CancellationToken ct) => Task.FromResult(File.Exists(PathOf(key)));

    /// <inheritdoc />
    public Task DeleteAsync(string key, CancellationToken ct)
    {
        var path = PathOf(key);
        if (File.Exists(path))
        {
            File.Delete(path);
        }

        return Task.CompletedTask;
    }

    /// <inheritdoc />
    public Task CopyAsync(string sourceKey, string destinationKey, CancellationToken ct)
    {
        var src = PathOf(sourceKey);
        if (!File.Exists(src))
        {
            throw new ObjectNotFoundException(sourceKey);
        }

        var dst = PathOf(destinationKey);
        Directory.CreateDirectory(Path.GetDirectoryName(dst)!);
        File.Copy(src, dst, overwrite: true);
        return Task.CompletedTask;
    }

    /// <inheritdoc />
    public Task<IReadOnlyList<string>> ListAsync(string prefix, CancellationToken ct)
    {
        IReadOnlyList<string> keys = Directory.EnumerateFiles(_root, "*", SearchOption.AllDirectories)
            .Select(p => Path.GetRelativePath(_root, p).Replace(Path.DirectorySeparatorChar, '/'))
            .Where(k => !k.StartsWith(TempDir + "/", StringComparison.Ordinal) && k.StartsWith(prefix, StringComparison.Ordinal))
            .Order(StringComparer.Ordinal)
            .ToList();
        return Task.FromResult(keys);
    }
}
