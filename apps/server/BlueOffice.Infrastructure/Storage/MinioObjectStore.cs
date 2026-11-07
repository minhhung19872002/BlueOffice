using BlueOffice.Domain;
using Minio;
using Minio.DataModel.Args;

namespace BlueOffice.Infrastructure.Storage;

/// <summary>S3-compatible object store backed by MinIO (Minio .NET SDK, Apache-2.0).</summary>
public sealed class MinioObjectStore(IMinioClient client, string bucket) : IObjectStore
{
    private readonly SemaphoreSlim _bucketGate = new(1, 1);
    private bool _bucketReady;

    private async Task EnsureBucketAsync(CancellationToken ct)
    {
        if (_bucketReady)
        {
            return;
        }

        await _bucketGate.WaitAsync(ct);
        try
        {
            if (!_bucketReady)
            {
                if (!await client.BucketExistsAsync(new BucketExistsArgs().WithBucket(bucket), ct))
                {
                    await client.MakeBucketAsync(new MakeBucketArgs().WithBucket(bucket), ct);
                }

                _bucketReady = true;
            }
        }
        finally
        {
            _bucketGate.Release();
        }
    }

    /// <inheritdoc />
    public async Task<long> PutAsync(string key, Stream content, string contentType, CancellationToken ct)
    {
        ObjectKeys.Check(key);
        await EnsureBucketAsync(ct);
        Stream source = content;
        FileStream? spool = null;
        try
        {
            if (!content.CanSeek)
            {
                // The SDK needs the length up front: spool unknown-length streams to a temp file.
                spool = new FileStream(Path.GetTempFileName(), FileMode.Open, FileAccess.ReadWrite, FileShare.None, 81920, FileOptions.DeleteOnClose);
                await content.CopyToAsync(spool, ct);
                spool.Position = 0;
                source = spool;
            }

            var size = source.Length - source.Position;
            await client.PutObjectAsync(
                new PutObjectArgs().WithBucket(bucket).WithObject(key).WithStreamData(source).WithObjectSize(size).WithContentType(contentType),
                ct);
            return size;
        }
        finally
        {
            if (spool is not null)
            {
                await spool.DisposeAsync();
            }
        }
    }

    /// <inheritdoc />
    public async Task<Stream> OpenReadAsync(string key, CancellationToken ct)
    {
        ObjectKeys.Check(key);
        await EnsureBucketAsync(ct);
        var spool = new FileStream(Path.GetTempFileName(), FileMode.Open, FileAccess.ReadWrite, FileShare.None, 81920, FileOptions.DeleteOnClose);
        try
        {
            await client.GetObjectAsync(
                new GetObjectArgs().WithBucket(bucket).WithObject(key).WithCallbackStream((s, c) => s.CopyToAsync(spool, c)),
                ct);
            spool.Position = 0;
            return spool;
        }
        catch (Minio.Exceptions.ObjectNotFoundException)
        {
            await spool.DisposeAsync();
            throw new Domain.ObjectNotFoundException(key);
        }
        catch
        {
            await spool.DisposeAsync();
            throw;
        }
    }

    /// <inheritdoc />
    public async Task<bool> ExistsAsync(string key, CancellationToken ct)
    {
        await EnsureBucketAsync(ct);
        try
        {
            await client.StatObjectAsync(new StatObjectArgs().WithBucket(bucket).WithObject(ObjectKeys.Check(key)), ct);
            return true;
        }
        catch (Minio.Exceptions.ObjectNotFoundException)
        {
            return false;
        }
    }

    /// <inheritdoc />
    public async Task DeleteAsync(string key, CancellationToken ct)
    {
        await EnsureBucketAsync(ct);
        await client.RemoveObjectAsync(new RemoveObjectArgs().WithBucket(bucket).WithObject(ObjectKeys.Check(key)), ct);
    }

    /// <inheritdoc />
    public async Task CopyAsync(string sourceKey, string destinationKey, CancellationToken ct)
    {
        await EnsureBucketAsync(ct);
        var src = new CopySourceObjectArgs().WithBucket(bucket).WithObject(ObjectKeys.Check(sourceKey));
        await client.CopyObjectAsync(
            new CopyObjectArgs().WithBucket(bucket).WithObject(ObjectKeys.Check(destinationKey)).WithCopyObjectSource(src),
            ct);
    }

    /// <inheritdoc />
    public async Task<IReadOnlyList<string>> ListAsync(string prefix, CancellationToken ct)
    {
        await EnsureBucketAsync(ct);
        var keys = new List<string>();
        await foreach (var item in client.ListObjectsEnumAsync(
            new ListObjectsArgs().WithBucket(bucket).WithPrefix(prefix).WithRecursive(true), ct))
        {
            keys.Add(item.Key);
        }

        keys.Sort(StringComparer.Ordinal);
        return keys;
    }
}
