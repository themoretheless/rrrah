// Explicit in-memory ClrMD adapter; never loads a memory dump or external process.
using System;using System.Collections.Generic;using System.IO;using System.Linq;using System.Reflection;
namespace Microsoft.Diagnostics.Runtime {
 public class ClrType { public bool IsString; }
 public class ClrObject { public ulong Address;public int Gen=2;public bool Present=true; public ClrType Type=new ClrType(); public string Text="";public List<ClrObject> Refs=new List<ClrObject>();public string AsString(int limit)=>Text;public IEnumerable<ClrObject> EnumerateReferences()=>Refs; }
 public class ClrSegment {public ulong Gen2Length=65536;public int GetGeneration(ClrObject o)=>o.Gen;}
 public class ClrHeap {public List<ClrObject> Objects=new List<ClrObject>();public IEnumerable<ClrSegment> Segments=>new[]{new ClrSegment()};public IEnumerable<ClrObject> EnumerateObjects()=>Objects;public ClrSegment GetSegmentByAddress(ulong address)=>Objects.Any(o=>o.Address==address&&o.Present)?new ClrSegment():null;}
 public class ClrRuntime {public ClrHeap Heap;}
 public class ClrInfo {public ClrRuntime Runtime;public ClrRuntime CreateRuntime()=>Runtime;}
 public class DataTarget:IDisposable {public static ClrHeap Fixture;public static bool Empty;public List<ClrInfo> ClrVersions=>Empty?new List<ClrInfo>():new List<ClrInfo>{new ClrInfo{Runtime=new ClrRuntime{Heap=Fixture}}};public static DataTarget LoadCrashDump(string p)=>new DataTarget();public static DataTarget LoadCoreDump(string p)=>new DataTarget();public void Dispose(){} }
}
#if REVIEW_WINDOWS
namespace StringDedupAnalyzer { public static class RuntimeInformation {public static bool IsOSPlatform(System.Runtime.InteropServices.OSPlatform p)=>p==System.Runtime.InteropServices.OSPlatform.Windows;} }
#endif
public static class ReviewHarness {
 static MethodInfo Entry=>typeof(StringDedupAnalyzer.Program).GetMethod("Main",BindingFlags.NonPublic|BindingFlags.Static);
 static string Invoke(string[] args){var previous=Console.Out;var output=new StringWriter();try{Console.SetOut(output);Entry.Invoke(null,new object[]{args});return output.ToString();}catch(TargetInvocationException e){throw e.InnerException;}finally{Console.SetOut(previous);}}
 static void Equal<T>(T a,T b){if(!object.Equals(a,b))throw new Exception($"expected {b}; got {a}");}
 static void Test(string name,Action f){f();Console.WriteLine("PASS "+name);}
 static Microsoft.Diagnostics.Runtime.ClrObject Str(ulong addr,int gen=2)=>new Microsoft.Diagnostics.Runtime.ClrObject{Address=addr,Gen=gen,Type=new Microsoft.Diagnostics.Runtime.ClrType{IsString=true},Text=new string('A',15)};
 static string Counts(int copies,bool same=false,int gen=2,bool present=true){var a=Str(1);var b=same?a:Str(2,gen);b.Present=present;var holder=new Microsoft.Diagnostics.Runtime.ClrObject{Address=3};holder.Refs.Add(a);for(int i=0;i<copies;i++)holder.Refs.Add(b);var objects=new List<Microsoft.Diagnostics.Runtime.ClrObject>{a,holder};if(!same)objects.Add(b);Microsoft.Diagnostics.Runtime.DataTarget.Fixture=new Microsoft.Diagnostics.Runtime.ClrHeap{Objects=objects};return Invoke(new[]{"owned-adapter-no-dump"}).Trim().Split('\n').Last().Trim();}
 public static void Main(){
 Test("usage_without_dump",()=>Equal(Invoke(Array.Empty<string>()).Trim(),"dotnet run dmp"));
#if REVIEW_WINDOWS
 Test("two_distinct_same_content_strings",()=>Equal(Counts(1),"65536,60,2,60,2,30,1"));
 Test("same_duplicate_object_three_references_counted_three_times",()=>Equal(Counts(3),"65536,60,2,120,4,90,3"));
 Test("same_physical_object_references_not_duplicates",()=>Equal(Counts(3,true),"65536,30,1,120,4,0,0"));
 Test("generation_one_reference_excluded",()=>Equal(Counts(3,false,1),"65536,30,1,30,1,0,0"));
 Test("unmapped_reference_excluded",()=>Equal(Counts(3,false,2,false),"65536,30,1,30,1,0,0"));
 Test("no_clr_runtime_index_error",()=>{Microsoft.Diagnostics.Runtime.DataTarget.Empty=true;try{Invoke(new[]{"owned"});throw new Exception("missing expected index error");}catch(ArgumentOutOfRangeException){}finally{Microsoft.Diagnostics.Runtime.DataTarget.Empty=false;}});
#else
 Test("actual_macos_dump_path_unsupported_before_loader",()=>{try{Invoke(new[]{"owned-no-existing-dump"});throw new Exception("expected unsupported platform");}catch(PlatformNotSupportedException){}});
#endif
 }
}
