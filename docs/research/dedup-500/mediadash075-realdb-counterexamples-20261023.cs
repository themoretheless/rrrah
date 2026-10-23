// Added only to the isolated pinned original test project, not upstream production.
using System;
using System.IO;
using System.Security.Cryptography;
using System.Threading;
using System.Threading.Tasks;
using Jellyfin.Plugin.MediaDash.Data;
using Jellyfin.Plugin.MediaDash.Probing;
using Microsoft.Extensions.Logging.Abstractions;
using Xunit;

namespace Jellyfin.Plugin.MediaDash.Tests;

public sealed class RrrahFileHasherCounterexamples
{
    [Fact]
    public async Task RestoredMetadataReturnsStaleDigestWithRealSqlite()
    {
        var dir = Path.Combine(Path.GetTempPath(), "rrrah-realdb-" + Guid.NewGuid());
        Directory.CreateDirectory(dir);
        try
        {
            var path = Path.Combine(dir, "owned.bin");
            File.WriteAllBytes(path, new byte[] { 1, 2, 3, 4 });
            var stamp = File.GetLastWriteTimeUtc(path);
            var db = new MediaDashDb(Path.Combine(dir, "cache.db"));
            var hasher = new FileHasher(db, NullLogger<FileHasher>.Instance);
            var original = await hasher.HashAsync(path, CancellationToken.None);
            Assert.Equal(Convert.ToHexString(SHA256.HashData(File.ReadAllBytes(path))).ToLowerInvariant(), original);
            File.WriteAllBytes(path, new byte[] { 5, 6, 7, 8 });
            File.SetLastWriteTimeUtc(path, stamp);
            Assert.Equal(stamp, File.GetLastWriteTimeUtc(path));
            Assert.Equal(4, new FileInfo(path).Length);
            var stale = await hasher.HashAsync(path, CancellationToken.None);
            Assert.Equal(original, stale);
            Assert.NotEqual(Convert.ToHexString(SHA256.HashData(File.ReadAllBytes(path))).ToLowerInvariant(), stale);
        }
        finally
        {
            Microsoft.Data.Sqlite.SqliteConnection.ClearAllPools();
            Directory.Delete(dir, recursive: true);
        }
    }

    [Fact]
    public async Task CancelledWarmHitReturnsDigestButColdMissThrowsWithRealSqlite()
    {
        var dir = Path.Combine(Path.GetTempPath(), "rrrah-realdb-" + Guid.NewGuid());
        Directory.CreateDirectory(dir);
        try
        {
            var path = Path.Combine(dir, "owned.bin");
            File.WriteAllBytes(path, new byte[] { 1, 2, 3, 4 });
            var db = new MediaDashDb(Path.Combine(dir, "cache.db"));
            var hasher = new FileHasher(db, NullLogger<FileHasher>.Instance);
            var original = await hasher.HashAsync(path, CancellationToken.None);
            Assert.NotNull(original);
            using var cts = new CancellationTokenSource();
            cts.Cancel();
            Assert.Equal(original, await hasher.HashAsync(path, cts.Token));
            var cold = new FileHasher(new MediaDashDb(Path.Combine(dir, "cold.db")), NullLogger<FileHasher>.Instance);
            await Assert.ThrowsAnyAsync<OperationCanceledException>(() => cold.HashAsync(path, cts.Token));
        }
        finally
        {
            Microsoft.Data.Sqlite.SqliteConnection.ClearAllPools();
            Directory.Delete(dir, recursive: true);
        }
    }
}
