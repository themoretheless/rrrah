"""Directly invoke all original C# test methods plus boundaries on real ImageSharp1.0.2."""
from pathlib import Path
import subprocess,json,hashlib
R=Path(__file__).resolve().parents[1];W=R/'.dedup-work/dupimage370-20261016';D=R/'docs/research/dedup-500';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
templates=R/'scripts/research/dupimage370-20261016'
for name in ['Program.cs','Harness.csproj']:assert (templates/name).read_bytes()==(W/'harness'/name).read_bytes()
tree=json.loads((D/'evidence/Quickshot--DupImageLib/tree.json').read_text());pins={}
for entry in tree['tree']:
 if entry['type']!='blob':continue
 p=W/'source'/entry['path'];b=p.read_bytes();assert hashlib.sha1(b'blob '+str(len(b)).encode()+b'\0'+b).hexdigest()==entry['sha'];pins[str(p.relative_to(R))]=h(p)
exe=W/'harness/bin/Release/net8.0/Harness.dll';results=[];logs={}
for i in [1,2]:
 log=W/f'native-v3-{i}.json';err=W/f'native-v3-{i}.stderr';assert not log.exists() and not err.exists()
 run=subprocess.run(['/usr/local/share/dotnet/dotnet',str(exe)],cwd=W/'source/DupImageLib.Tests',capture_output=True,text=True,timeout=60);err.write_text(run.stderr);assert run.returncode==0,run.stdout+run.stderr
 d=json.loads(run.stdout);assert len(d['original'])==16 and len(d['controls'])==16 and all(x['passed'] for x in d['original']+d['controls']);log.write_text(json.dumps(d,indent=2)+'\n');results.append(d)
 for p in [log,err]:logs[str(p.relative_to(R))]=h(p)
assert results[0]==results[1]
assets={}
for p in [W/'source/DupImageLib/obj/project.assets.json',W/'source/DupImageLib.Tests/obj/project.assets.json',W/'harness/obj/project.assets.json']:
 d=json.loads(p.read_text());assert 'SixLabors.ImageSharp/1.0.2' in d['libraries'];assets[str(p.relative_to(R))]={'sha256':h(p),'packages':sorted(d['libraries'])}
runtime_pins={str(p.relative_to(R)):h(p) for p in sorted((W/'harness/bin/Release/net8.0').glob('*')) if p.suffix in ['.dll','.json','.pdb']}
p=D/'dupimage370-native-controls-20261016.json';assert not p.exists();p.write_text(json.dumps({'status':'verified_original_method_compatibility_invocation_and_native_boundaries','producer_sha256':h(Path(__file__)),'source_pins':pins,'harness_pins':{str(p.relative_to(R)):h(p) for p in [templates/'Program.cs',templates/'Harness.csproj',W/'harness/Program.cs',W/'harness/Harness.csproj',W/'global.json',W/'harness-build.log',W/'harness-build-v2.log']},'runtime_pins':runtime_pins,'assets':assets,'logs':logs,'runs':2,'original_cases_per_run':16,'boundary_cases_per_run':16,'rows':results,'original_cli_attempt':{'result':'aborted_missing_X64_dotnet_host','sdk':'11.0.100-preview.5.26302.115','target':'net5','trx':str((W/'original-results/original.trx').relative_to(R)),'trx_sha256':h(W/'original-results/original.trx')},'scope':'Whole unchanged original netstandard2.0productionproject withrealImageSharp1.0.2 andoriginalcompiled net5testassembly/realXunit2.4.1assertions. SDK10.0.402 buildsownednet8harness; runtime8.0.32ARM64. All11Fact+3Theorycases+2ImageSharpFact directlyinvokedtwice byreflection. InlineData scalarvalues explicitlyConvert.ChangeType todeclaredmethodparameter matchingtestbinding; originalmethods/assertions unchanged. 16additionalboundaries twice; onlyshortbuffer/null publictransformer injections, realPNGImageSharp for5solidimagehashcollisions/blackDCT/invalidimage/openstream.','limits':['Originaldotnet test/VSTest targetnet5 abortedmissingx64host; compatiblemethodinvocation isnotoriginalrunner/runtime5/platformqualification','No productioncodecopied, realcorpus precision/recall/formats/adoption/performance claim','Originaldependencyversion retained; packagewarnings inbuildlog, nosecurityremediationclaim','Earlierharnessfirstreflectionattemptfailedparameterbinding andfixedonlyownedharness; retainednative-1.stderr andv2/v3outputs']},indent=2)+'\n');print('Verified16originalcases+16boundaries eachtwice; compatibilityrunner only')
