#!/usr/bin/env python3
from pathlib import Path
import json,hashlib,subprocess,os
R=Path(__file__).resolve().parents[1];D=R/'docs/research';E=D/'dedup-500/evidence/nikvoronin--we-aint-same';F=Path('/private/tmp/rrrah-weaintsame488-20261014');F.mkdir(exist_ok=False);sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();s=(E/'02-Program.cs').read_text(encoding='utf-8-sig');helper=s[s.index('file class ImageInfo'):];h=F/'Program.cs'
pre='using CoenM.ImageHash;using System;using System.Linq;using System.Collections.Generic;using System.Numerics;using System.Diagnostics.CodeAnalysis;using System.Text.Json;\n'
main=r'''
var records=new List<object>();
void Check(bool value,string message){if(!value)throw new Exception(message);}
ulong a=0,b=63,c=4095;
var relation=new ImageHashComparer(90);Check(relation.Equals(a,b)&&relation.Equals(b,c)&&!relation.Equals(a,c),"nontransitive relation missing");
records.Add(new{name="nontransitive",ab=true,bc=true,ac=false});
ulong[][] orders=[[a,b,c],[a,c,b],[b,a,c],[b,c,a],[c,a,b],[c,b,a]];
foreach(var order in orders){var cmp=new ImageHashComparer(90);var groups=order.GroupBy(x=>x,cmp).Select(x=>x.ToArray()).ToArray();Check(groups.Length==(order[0]==b?1:2),"unexpected GroupBy representative behavior");records.Add(new{name="permutation",input=order,groups});}
foreach(var threshold in new[]{90.0,100.0,-1.0,double.NaN}){var cmp=new ImageHashComparer(threshold);bool identical=cmp.Equals(0,0);Check(identical==(threshold<100&&!double.IsNaN(threshold)),"threshold self result");records.Add(new{name="threshold-self",threshold=threshold.ToString(System.Globalization.CultureInfo.InvariantCulture),identical});}
var hashes=new List<ulong>();ulong state=0x123456789abcdefUL;
for(int i=0;i<128;i++){state^=state<<13;state^=state>>7;state^=state<<17;hashes.Add(state);}
foreach(int n in new[]{16,32,64,128}){var cmp=new ImageHashComparer(90);CompareHash.Calls=0;var groups=hashes.Take(n).GroupBy(x=>x,cmp).ToArray();long calls=CompareHash.Calls;Check(groups.Length==n&&calls==(long)n*(n-1)/2,"quadratic distinct grouping");records.Add(new{name="unique-scaling",n,groups=groups.Length,calls,expected=(long)n*(n-1)/2});}
var untouched=new ImageHashComparer(90);Check(untouched.MinSim==200&&untouched.MaxSim==0,"initial range changed");records.Add(new{name="no-comparison-range",minimum=untouched.MinSim,maximum=untouched.MaxSim});
Console.WriteLine(JsonSerializer.Serialize(records));
'''
adapter='\nnamespace CoenM.ImageHash { public static class CompareHash { public static long Calls; public static double Similarity(ulong x,ulong y){Calls++;return 100.0*(64-System.Numerics.BitOperations.PopCount(x^y))/64.0;} } }\n'
h.write_text(pre+main+'\n'+helper+adapter);(F/'global.json').write_text(json.dumps({'sdk':{'version':'8.0.425','rollForward':'disable'}}));(F/'Controls.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net8.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings><Nullable>enable</Nullable><UseSharedCompilation>false</UseSharedCompilation></PropertyGroup></Project>');(F/'NuGet.Config').write_text('<configuration><packageSources><clear /></packageSources></configuration>')
env=os.environ.copy();env.update(DOTNET_CLI_HOME=str(F/'cli-home'),DOTNET_CLI_TELEMETRY_OPTOUT='1',DOTNET_SKIP_FIRST_TIME_EXPERIENCE='1',DOTNET_GENERATE_ASPNET_CERTIFICATE='false',NUGET_PACKAGES=str(F/'packages'),MSBuildEnableWorkloadResolver='false')
O=D/'dedup-weaintsame488-controls-20261014.json';assert not O.exists();d=dict(status='building_helpers',producer_pid=os.getpid(),producer_sha256=sha(__file__),source_sha256=sha(E/'02-Program.cs'),helper_sha256=hashlib.sha256(helper.encode()).hexdigest(),harness=str(h),harness_sha256=sha(h),source=str(F),scope='Original ImageInfo/ImageHashComparer helper bodies unchanged, compiled with installed actual .NET8SDK and real LINQ GroupBy. Explicit scalar64bit-Hamming percentage adapter replaces unavailable ImageHash1.3.6 dependency. No whole original CLI/MyPictures traversal/cache/media/private input/network or actual ImageSharp/perceptual hash execution. Grouping findings conditional on stated adapter; not ImageHash API/runtime qualification.')
def save():
 t=O.with_suffix('.tmp');t.write_text(json.dumps(d,indent=2)+'\n');t.replace(O)
save()
commands=[('restore',['dotnet','restore','Controls.csproj','--configfile',str(F/'NuGet.Config'),'--ignore-failed-sources']),('build',['dotnet','build','Controls.csproj','--no-restore','-c','Release','-m:1','-p:UseSharedCompilation=false']),('controls',['dotnet',str(F/'bin/Release/net8.0/Controls.dll')])]
for label,cmd in commands:
 with (F/(label+'.log')).open('x')as log:
  p=subprocess.Popen(cmd,cwd=F,env=env,stdout=log,stderr=subprocess.STDOUT);d.update(status=label,child_pid=p.pid);save();rc=p.wait()
 d[label+'_returncode']=rc;save()
 if rc:d.update(status='terminal_'+label+'_failure');save();raise SystemExit(rc)
rows=json.loads((F/'controls.log').read_text());assert len(rows)==16;d.update(status='passed_pending_independent_audit',rows=rows,output_pins={str(p):sha(p)for p in F.glob('*.log')},executable=str(F/'bin/Release/net8.0/Controls.dll'),executable_sha256=sha(F/'bin/Release/net8.0/Controls.dll'));assert sha(E/'02-Program.cs')==d['source_sha256'];save();print('16actual .NET helper/grouping controls passed with explicit scalar distance adapter')
