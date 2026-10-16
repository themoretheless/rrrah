"""Fresh standalone Cargo gate for whole current modules after old temporary rlibs vanished."""
from pathlib import Path
import subprocess,json,hashlib,shutil,re,tomllib
R=Path(__file__).resolve().parents[1];W=R/'.dedup-work/current-pixel-sequence-v3-20261016';D=R/'docs/research';W.mkdir(exist_ok=False);(W/'.gitignore').write_text('*\n');h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();sources={};count=0
for module in ['pixels','linear','sequence']:
 for kind,prefix in [('src',''),('tests','test_')]:
  p=R/f'crates/rrrah-dedup/{kind}/{module}.rs';shutil.copyfile(p,W/(prefix+module+'.rs'));sources[str(p.relative_to(R))]=h(p);count+=p.read_text().count('#[test]')
local=R/'crates/rrrah-dedup/src/local.rs';s=local.read_text();error=s[s.index('#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]'):s.index('\n/// Extract deterministic')];helper=s[s.index('pub(crate) fn reserve_slot'):s.index('\n#[allow(clippy::too_many_lines)]')];sources[str(local.relative_to(R))]=h(local)
harness='extern crate self as rrrah_dedup;\nmod local {\n'+error+'\n'+helper+'\n}\n'+''.join(f'#[path="{m}.rs"]pub mod {m};\n#[path="test_{m}.rs"]mod {m}_controls;\n' for m in ['pixels','linear','sequence']);(W/'harness.rs').write_text(harness)
deps=tomllib.loads((R/'Cargo.toml').read_text())['workspace']['dependencies'];assert deps['blake3']=='1.8' and deps['thiserror']=='2.0'
(W/'Cargo.toml').write_text('[package]\nname="owned-current-pixel-sequence-controls"\nversion="0.0.0"\nedition="2024"\n[workspace]\n[lib]\npath="harness.rs"\n[dependencies]\nblake3="1.8"\nthiserror="2.0"\n')
cmd=['cargo','+1.98.1','test','--offline','--manifest-path',str(W/'Cargo.toml'),'--target-dir',str(W/'target'),'--jobs','2','--lib','--no-run','--message-format=json']
with (W/'build.log').open('x') as f:result=subprocess.run(cmd,cwd=W,stdout=f,stderr=subprocess.STDOUT)
(W/'build-status.json').write_text(json.dumps({'command':cmd,'exit_code':result.returncode},indent=2)+'\n');assert result.returncode==0,'See ownedbuild.log; do not overwrite failed evidence'
artifacts=[]
for line in (W/'build.log').read_text().splitlines():
 try:item=json.loads(line)
 except ValueError:continue
 if item.get('reason')=='compiler-artifact' and item.get('executable'):artifacts.append(item)
assert len(artifacts)==1;exe=Path(artifacts[0]['executable']);listing=subprocess.check_output([str(exe),'--list'],text=True);(W/'inventory.log').write_text(listing);names=re.findall(r'^(.*): test$',listing,re.M);assert len(names)==len(set(names))==count==22
for i in [1,2]:
 run=subprocess.run([str(exe),'--test-threads=2'],cwd=W,capture_output=True,text=True);(W/f'tests-{i}.log').write_text(run.stdout+run.stderr);assert run.returncode==0 and f'{count} passed; 0 failed; 0 ignored; 0 measured; 0 filtered out' in run.stdout
 assert sorted(re.findall(r'^test (\S+) \.\.\. ok$',run.stdout,re.M))==sorted(names)
assert all(h(R/p)==sha for p,sha in sources.items());assert 'sequence_controls::ordered_subsets_do_not_establish_complete_sequence_or_timeline_identity' in names
p=D/'dedup-current-pixel-sequence-v3-20261016.json';assert not p.exists();p.write_text(json.dumps({'status':'fresh_passed_complete_current_pixel_linear_sequence_scope','passed_per_run':22,'runs':2,'ignored':0,'filtered':0,'inventory':names,'source_pins':sources,'workspace_dependency_constraints':{k:deps[k] for k in ['blake3','thiserror']},'focused_lock':str((W/'Cargo.lock').relative_to(R)),'focused_lock_sha256':h(W/'Cargo.lock'),'focused_lock_packages':tomllib.loads((W/'Cargo.lock').read_text())['package'],'binary':str(exe),'binary_sha256':h(exe),'harness':str(W/'harness.rs'),'harness_sha256':h(W/'harness.rs'),'producer_sha256':h(Path(__file__)),'compiler':subprocess.check_output(['rustc','+1.98.1','-vV'],text=True),'build_command':cmd,'logs':{str(p.relative_to(R)):h(p) for p in [W/'build.log',W/'build-status.json',W/'inventory.log',W/'tests-1.log',W/'tests-2.log',W/'Cargo.toml']},'scope':'All3wholeunchangedcurrentpixels/linear/sequence modules and3wholeintegrationfiles, selectedunchangedLocalError/reserve_slot helperdefinitions. NewpublicAPIregression all30properorderedsubsets of5frames atPages/Animation, bothdirections/bothsequenceandtimenline APIs:240negativecomparisons+4identitypositives+2emptyrefusals. Complete22testinventory twice44passes withactualCargo/realBLAKE3/thiserrordependencies matchingrootdeclaredconstraints; focusedlockresolvedoffline separately, notrootCargo.lockparity.','limits':['Priorv2standaloneattemptstoppedbeforecompilebecauseoldtemporaryrlibsremoved; preserved, no passingoldbinaryinferred','StandalonefocusedCargo/project, no complete rrrah-dedup/default+decode/fullworkspace/platform/decoder/imagevideoquality/performance/currentallcases qualification','Oldv1source/runtimeproof remains historical; thisfreshgate validateslatestsource/testpins only']},indent=2)+'\n');print('Fresh22currentpixel/linear/sequence tests twice44passes; orderedsubset identity regression included')
