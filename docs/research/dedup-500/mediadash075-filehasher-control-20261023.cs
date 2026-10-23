// Original complete FileHasher.cs is compiled alongside this bounded harness.
// Persistence and logging adapters below are not the original MediaDash DB/host.
using System.Security.Cryptography;
using System.Text.Json;
using Jellyfin.Plugin.MediaDash.Probing;

var dir = Path.Combine(Path.GetTempPath(), "rrrah-mediadash-" + Guid.NewGuid());
Directory.CreateDirectory(dir);
try
{
    var path = Path.Combine(dir, "owned.bin");
    File.WriteAllBytes(path, new byte[] { 1, 2, 3, 4 });
    var stamp = File.GetLastWriteTimeUtc(path);
    var db = new Jellyfin.Plugin.MediaDash.Data.MediaDashDb();
    var hasher = new FileHasher(db, new Microsoft.Extensions.Logging.NullLogger<FileHasher>());
    var first = await hasher.HashAsync(path, CancellationToken.None);
    var expected = Convert.ToHexString(SHA256.HashData(File.ReadAllBytes(path))).ToLowerInvariant();
    if (first != expected) throw new Exception("Baseline hash mismatch");
    File.WriteAllBytes(path, new byte[] { 5, 6, 7, 8 });
    File.SetLastWriteTimeUtc(path, stamp);
    if (File.GetLastWriteTimeUtc(path) != stamp) throw new Exception("mtime restoration failed");
    var second = await hasher.HashAsync(path, CancellationToken.None);
    var actual = Convert.ToHexString(SHA256.HashData(File.ReadAllBytes(path))).ToLowerInvariant();
    if (second != first || second == actual) throw new Exception("Expected stale-cache counterexample absent");
    using var cts = new CancellationTokenSource();
    cts.Cancel();
    var cancelledHit = await hasher.HashAsync(path, cts.Token);
    if (cancelledHit != first) throw new Exception("Expected cancelled cache hit absent");
    var fresh = new FileHasher(new(), new Microsoft.Extensions.Logging.NullLogger<FileHasher>());
    var cancelledMissThrew = false;
    try { await fresh.HashAsync(path, cts.Token); }
    catch (OperationCanceledException) { cancelledMissThrew = true; }
    if (!cancelledMissThrew) throw new Exception("Cache-miss cancellation did not throw");
    Console.WriteLine(JsonSerializer.Serialize(new {
        controls = 4, passed = 4, baselineSha256 = first, mutatedSha256 = actual,
        restoredMetadataReturnsStaleHash = second == first,
        cancelledCacheHitReturnsHash = cancelledHit == first, cancelledCacheMissThrows = cancelledMissThrew,
        scope = "Complete pinned original FileHasher body, real .NET SHA256 and owned filesystem; in-memory cache/log adapters, not original DB or Jellyfin runtime. Counterexamples asserted, not repaired."
    }));
}
finally { Directory.Delete(dir, recursive: true); }

namespace Jellyfin.Plugin.MediaDash.Data
{
    public sealed class MediaDashDb
    {
        private readonly Dictionary<(string, long, long), string> values = new();
        public string? GetCachedHash(string path, long size, long mtime) => values.GetValueOrDefault((path, size, mtime));
        public void StoreHash(string path, long size, long mtime, string hash) => values[(path, size, mtime)] = hash;
    }
}
namespace Microsoft.Extensions.Logging
{
    public interface ILogger<T> { }
    public sealed class NullLogger<T> : ILogger<T> { }
    public static class LoggerExtensions
    {
        public static void LogDebug<T>(this ILogger<T> logger, Exception error, string message, params object[] args) { }
    }
}
