using DupeSweep;
using System.Diagnostics;
using System.Reflection;
using System.Security.Cryptography;
using System.Text.Json;

internal static class Probe
{
    private static string Root = "";
    private static readonly Dictionary<string, object?> Observations = [];
    private static void Require(bool value) { if (!value) throw new Exception("control assertion failed"); }
    private static string Dir(string name) => Directory.CreateDirectory(Path.Combine(Root, name)).FullName;
    private static FileEntry Entry(string path) { var f = new FileInfo(path); return new(path, f.Length, f.LastWriteTimeUtc); }
    private static async Task<DuplicateResolution> EqualPair(string directory)
    {
        string a = Path.Combine(directory, "keep"), b = Path.Combine(directory, "duplicate");
        File.WriteAllText(a, "same payload"); File.WriteAllText(b, "same payload");
        var result = await new DuplicateFinder(new ScanOptions { Parallelism = 2 }).FindGroupsAsync([Entry(a), Entry(b)], CancellationToken.None);
        Require(result.Groups.Count == 1);
        return KeepSelector.Resolve(result.Groups[0], KeepStrategy.First);
    }
    private static async Task Case(string name, Func<Task<object?>> run)
    { Observations[name] = await run(); Console.WriteLine("PASS " + name); }
    private static Task<object?> Value(object? x) => Task.FromResult(x);
    private static void Delete(DuplicateResolution resolution)
    {
        var method = typeof(DupeSweep.Program).GetMethod("ApplyResolutions", BindingFlags.NonPublic | BindingFlags.Static)!;
        method.Invoke(null, [new ScanOptions { Apply = ApplyMode.Delete, Quiet = true }, new List<DuplicateResolution> { resolution }]);
    }
    public static async Task Main(string[] args)
    {
        Root = args.Single(); Require(Directory.Exists(Root));
        await Case("same_64KiB_prefix_different_tail_rejected_by_actual_full_hash", async () => {
            string d = Dir("tail"), a = Path.Combine(d,"a"), b = Path.Combine(d,"b");
            byte[] data = new byte[65537]; data[0] = 3; File.WriteAllBytes(a,data); data[^1] = 1; File.WriteAllBytes(b,data);
            Require(Hashing.ComputeQuickHash(a,data.Length,default) == Hashing.ComputeQuickHash(b,data.Length,default));
            var result = await new DuplicateFinder(new()).FindGroupsAsync([Entry(a),Entry(b)],default);
            Require(result.Groups.Count == 0 && result.Warnings.Count == 0); return "Actualfullhash separates prefix collision";
        });
        await Case("actual_full_hash_matches_independent_byte_array_SHA256", async () => {
            string p = Path.Combine(Dir("sha"),"bytes"); byte[] bytes = Enumerable.Range(0,10000).Select(i=>(byte)(i%251)).ToArray(); File.WriteAllBytes(p,bytes);
            Require(await Hashing.ComputeFullHashAsync(p,default) == Convert.ToHexString(SHA256.HashData(bytes))); return bytes.Length;
        });
        await Case("pre_cancelled_empty_quick_hash_returns_digest_without_cancellation", () => {
            string p=Path.Combine(Dir("cancel-empty"),"empty"); File.WriteAllBytes(p,[]); using var c=new CancellationTokenSource();c.Cancel();
            Require(Hashing.ComputeQuickHash(p,0,c.Token)==Convert.ToHexString(SHA256.HashData([]))); return Value("QuickHash zero-size branch, not wholefinder cancellation");
        });
        await Case("stale_zero_fileLength_makes_nonempty_quick_hash_empty", () => {
            string p=Path.Combine(Dir("zero"),"payload");File.WriteAllText(p,"nonempty");
            Require(Hashing.ComputeQuickHash(p,0,default)==Convert.ToHexString(SHA256.HashData([])));return Value("Explicitstale metadata argument");
        });
        await Case("original_ReadFully_handles_legal_short_reads_and_cancel_before_IO", () => {
            var m=typeof(Hashing).GetMethod("ReadFully",BindingFlags.NonPublic|BindingFlags.Static)!;
            byte[] expected=Enumerable.Range(0,16).Select(i=>(byte)i).ToArray(),buffer=new byte[16];using var stream=new ShortStream(expected);
            Require((int)m.Invoke(null,[stream,buffer,CancellationToken.None])! ==16 && buffer.SequenceEqual(expected));
            using var c=new CancellationTokenSource();c.Cancel();using var stopped=new ShortStream(expected);bool refused=false;
            try {m.Invoke(null,[stopped,new byte[16],c.Token]);} catch(TargetInvocationException e) when(e.InnerException is OperationCanceledException){refused=true;}
            Require(refused && stopped.ReadCalls==0);return Value("Unchangedprivatemethod, explicitStream subclass capsReadat2bytes");
        });
        await Case("finder_publishes_old_file_sizes_after_equal_content_growth", async () => {
            string d=Dir("growth"),a=Path.Combine(d,"a"),b=Path.Combine(d,"b");File.WriteAllText(a,"same");File.WriteAllText(b,"same");var old=new[]{Entry(a),Entry(b)};
            File.WriteAllText(a,"equal grown payload");File.WriteAllText(b,"equal grown payload");var r=await new DuplicateFinder(new()).FindGroupsAsync(old,default);
            Require(r.Groups.Count==1 && r.Groups[0].FileSize==4 && new FileInfo(a).Length>4);return new {publishedSize=r.Groups[0].FileSize,actualSize=new FileInfo(a).Length};
        });
        await Case("missing_candidate_warning_is_not_carried_into_JSON_report", async () => {
            string d=Dir("missing");var resolution=await EqualPair(d);var entries=resolution.Group.Files.ToList();entries.Add(new FileEntry(Path.Combine(d,"absent"),entries[0].Length,DateTime.UtcNow));
            var r=await new DuplicateFinder(new()).FindGroupsAsync(entries,default);Require(r.Groups.Count==1 && r.Warnings.Count==1);
            string report=ReportWriter.WriteJson(r.Groups.Select(g=>KeepSelector.Resolve(g,KeepStrategy.First)).ToList());Require(!report.Contains("absent")&&!report.Contains("warnings"));return r.Warnings.Count;
        });
        await Case("quarantine_moves_post_scan_unique_rewrite_and_records_old_digest", async () => {
            string d=Dir("quarantine-rewrite");var r=await EqualPair(d);string victim=r.Duplicates.Single().FullPath;File.WriteAllText(victim,"new unique payload");
            var manifest=QuarantineService.Quarantine([r],Path.Combine(d,"q"));Require(!File.Exists(victim)&&File.ReadAllText(manifest[0].QuarantinePath)=="new unique payload");
            Require(manifest[0].Hash!=await Hashing.ComputeFullHashAsync(manifest[0].QuarantinePath,default));return "ExplicitbeforeQuarantine rewrite, unique bytes preservedinquarantine";
        });
        await Case("restore_accepts_changed_quarantined_bytes_without_manifest_hash_check", async () => {
            string d=Dir("restore-tamper");var r=await EqualPair(d);var manifest=QuarantineService.Quarantine([r],Path.Combine(d,"q"));File.WriteAllText(manifest[0].QuarantinePath,"changed quarantine payload");
            var result=RestoreService.Restore(manifest,false);Require(result.Restored==1&&File.ReadAllText(manifest[0].OriginalPath)=="changed quarantine payload");return "Explicitpayload edit, actualrestoremethod nohash/lengthvalidation";
        });
        await Case("actual_private_delete_removes_unique_post_scan_rewrite", async () => {
            var r=await EqualPair(Dir("delete-rewrite"));string victim=r.Duplicates.Single().FullPath;File.WriteAllText(victim,"unique now");Delete(r);Require(!File.Exists(victim)&&File.Exists(r.Keep.FullPath));return "ActualprivateApplyResolutions viareflection, beforecallrewrite onownedfixture";
        });
        await Case("actual_private_delete_does_not_revalidate_missing_keeper", async () => {
            var r=await EqualPair(Dir("delete-last"));File.Delete(r.Keep.FullPath);Delete(r);Require(!File.Exists(r.Keep.FullPath)&&!File.Exists(r.Duplicates.Single().FullPath));return "Explicitkeeperremovalbeforeactualprivateapply";
        });
        await Case("native_hardlink_paths_count_as_reclaimable_duplicate", async () => {
            string d=Dir("hardlinks"),a=Path.Combine(d,"a"),b=Path.Combine(d,"b");File.WriteAllText(a,"alias bytes");
            var start=new ProcessStartInfo("/bin/ln"){UseShellExecute=false};start.ArgumentList.Add(a);start.ArgumentList.Add(b);using var child=Process.Start(start)!;child.WaitForExit();Require(child.ExitCode==0);
            var entries=FileScanner.Enumerate([d],new()).ToList();var r=await new DuplicateFinder(new()).FindGroupsAsync(entries,default);
            Require(entries.Count==2&&r.Groups.Count==1&&r.Groups[0].ReclaimableBytes==11);return "RealMACHardlink, reportedlogical11bytes; noactualspace-reclaimedmeasurement";
        });
        await Case("native_directory_symlink_ancestor_cycle_terminates", () => {
            string d=Dir("cycle"),sub=Directory.CreateDirectory(Path.Combine(d,"sub")).FullName;File.WriteAllText(Path.Combine(d,"a"),"a");File.WriteAllText(Path.Combine(sub,"b"),"b");Directory.CreateSymbolicLink(Path.Combine(sub,"back"),d);
            var entries=FileScanner.Enumerate([d],new(){FollowSymlinks=true}).ToList();Require(entries.Count==2);return Value("ActualnativeDirEntry/linkresolution, noresolveradapter");
        });
        await Case("restore_existing_directory_throws_and_keeps_payload", () => {
            string d=Dir("restore-dir"),q=Path.Combine(d,"payload"),destination=Directory.CreateDirectory(Path.Combine(d,"dest")).FullName;File.WriteAllText(q,"payload");bool refused=false;
            try{RestoreService.Restore([new(destination,q,7,"declared hash")],false);}catch(IOException){refused=true;}Require(refused&&File.Exists(q)&&Directory.Exists(destination));return Value("Directorydestination refusal isIOException, not skippedentry");
        });
        await Case("nonfinite_minimum_size_values_are_accepted", () => Value(new {nan=CommandLine.Parse(["owned-root","--min-size","NaN"]).MinSizeBytes,infinity=CommandLine.Parse(["owned-root","--min-size","Infinity"]).MinSizeBytes}));
        await Case("duration_minutes_wrap_at_one_hour", () => {Require(Format.Duration(TimeSpan.FromSeconds(3605))=="00:05");return Value("Minutescomponent insteadtotalminutes");});
        Console.WriteLine(JsonSerializer.Serialize(new{status="passed",controls=Observations.Count,cases=Observations.Keys,observations=Observations,runtime=System.Runtime.InteropServices.RuntimeInformation.FrameworkDescription,scope="Actualunchangedcompleteproductionassembly onMacARM/net10.0.13. Controlledownedfixtures,explicitbetweenstageedits/shortreadstream/privatefunctionreflection; notambientrace/fullCLIinterleaving orotherplatform/performancequalification."}));
    }
    private sealed class ShortStream(byte[] bytes):MemoryStream(bytes)
    {
        public int ReadCalls {get;private set;}
        public override int Read(byte[] buffer,int offset,int count){ReadCalls++;return base.Read(buffer,offset,Math.Min(2,count));}
    }
}
