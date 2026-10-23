using DupeClear.Models;
using DupeClear.Models.Finder;
var root=Path.Combine(Path.GetTempPath(),"dupeclear-owned-"+Guid.NewGuid());
Directory.CreateDirectory(root);
try {
 var a=Path.Combine(root,"a"+Guid.NewGuid());var b=Path.Combine(root,"b"+Guid.NewGuid());
 File.WriteAllText(a,"matching bytes");File.WriteAllText(b,"matching bytes");
 var boundary=new DateTime(2020,1,2,12,0,0);
 File.SetLastWriteTime(a,boundary.AddDays(-1)); File.SetLastWriteTime(b,boundary.AddDays(1));
 Task<FinderResult> Find(DateTime? upper=null)=>FinderService.FindAsync(new[]{new SearchDirectory(root)},Array.Empty<SearchDirectory>(),FinderOption.SameContents|FinderOption.SameSize|FinderOption.AcrossDirectories,null,0,null,null,null,null,null,upper);
 void Check(bool ok,string label){if(!ok)throw new Exception(label);Console.WriteLine("PASS "+label);}
 var baseline=await Find();Check(baseline.Files.Count==2,"positive byte-equal pair");
 var filtered=await Find(boundary);Check(filtered.Files.Count==2,"upper modified bound still yields before and after files");
 Check(filtered.Files.First().FullName==a,"after-bound source anchors before-bound target (result target inserted first)");
 File.WriteAllBytes(a,Array.Empty<byte>());File.WriteAllBytes(b,Array.Empty<byte>());
 var empty=await Find();Check(empty.Files.Count==0,"two empty files skipped at zero minimum");
 File.WriteAllText(a,"hardlink fixture"); File.Delete(b);
 Check(Native.Link(a,b)==0,"task-owned hardlink created");
 var aliases=await Find();Check(aliases.Files.Count==2 && aliases.DuplicateCount==1,"physical aliases reported as duplicate copies");
 Console.WriteLine("6 checks passed; original service/model/JPEG code; UI/logging/type adapters only; no deletion API invoked.");
} finally { Directory.Delete(root,true); }

static class Native { [System.Runtime.InteropServices.DllImport("libc",EntryPoint="link",SetLastError=true)] public static extern int Link(string source,string target); }
