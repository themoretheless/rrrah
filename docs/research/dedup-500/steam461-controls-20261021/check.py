import pathlib,subprocess,json,hashlib
root=pathlib.Path.cwd();src=root/'.dedup-work/steam461-20261021/SteamSleuthData/Cache';out=root/'.dedup-work/steam461-controls-20261021';out.mkdir(exist_ok=True)
(out/'global.json').write_text('{"sdk":{"version":"10.0.402"}}')
(out/'control.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings><Nullable>enable</Nullable></PropertyGroup><ItemGroup>'+''.join('<Compile Include="'+str(src/p)+'" />' for p in ['Functionality/CacheBucket.cs','ObjectCacheProvider.cs'])+'</ItemGroup></Project>')
(out/'Program.cs').write_text('''using SteamSleuthData.Cache;
using System.Text.Json;
var first=new CacheBucket(30);var second=new CacheBucket(30);
var a=new object();var b=new object();
first.SetCachedObject(a,allowGarbageCollection:false);
if(!ReferenceEquals(first.GetCachedObject(),a))throw new Exception("initial strong reference missing");
second.SetCachedObject(b,allowGarbageCollection:true);
bool firstLost=first.GetCachedObject()==null;
if(!firstLost)throw new Exception("counterexample absent");
first.SetCachedObject(a,allowGarbageCollection:false);
bool secondLost=second.GetCachedObject()==null;
if(!secondLost)throw new Exception("reverse counterexample absent");
GC.KeepAlive(a);GC.KeepAlive(b);
var expiry=new CacheBucket(30);expiry.SetCachedObject(a,false);expiry.LastUpdated=DateTime.Now.AddSeconds(-60);
if(!expiry.IsExpired())throw new Exception("expiry control failed");
Console.WriteLine(JsonSerializer.Serialize(new {firstLostAfterOtherBucketWeakWrite=firstLost,secondLostAfterOtherBucketStrongWrite=secondLost,expiredAfter60Seconds=true,method="Original CacheBucket.cs and ObjectCacheProvider.cs compiled unchanged, net10.0 target adapter; isolated controls only, no MongoDB, HTTP or upstream program execution."}));
''')
p=subprocess.run(['dotnet','run','--project','control.csproj','--configuration','Release'],cwd=out,capture_output=True,text=True);assert p.returncode==0,p.stdout+p.stderr
r=json.loads(p.stdout.strip().splitlines()[-1]);r['source_sha256']={str(p):hashlib.sha256((src/p).read_bytes()).hexdigest() for p in ['Functionality/CacheBucket.cs','ObjectCacheProvider.cs']};r['sdk']='10.0.402';path=root/'docs/research/dedup-500/steam461-controls-20261021/result.json';path.write_text(json.dumps(r,indent=2)+'\n');print(path.read_text())
