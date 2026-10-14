#!/usr/bin/env python3
"""Fresh-process alternating actual public compare_files A/B; CPU, not shared-host wall."""
from pathlib import Path
import os,json,hashlib,subprocess,resource,time,statistics
R=Path(__file__).resolve().parents[1];D=R/'docs/research';G=D/'dedup-fused-exact-pair-controls-v3-20261014.json';g=json.loads(G.read_text());assert g['status']=='passed_pending_performance_and_independent_audit' and g['failed']==0
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();prior=json.loads((D/'dedup-gradient-unroll4-managed-focused-v2-20261014.json').read_text());P=D/'dedup-fused-exact-pair-experiment-20261014.json';plan=json.loads(P.read_text());S=Path(plan['source']);assert all(sha(k)==h for k,h in prior['dependency_pins'].items());assert all(sha(S/'baseline'/k)==h for k,h in plan['variants']['baseline'].items());assert all(sha(Path(g['source'])/k)==h for k,h in g['source_sha256'].items())
F=Path('/private/tmp/rrrah-fused-exact-pair-long-v4-20261014');F.mkdir(exist_ok=False);K=D/'dedup-fused-exact-pair-long-v4-20261014';K.mkdir(exist_ok=False);O=D/'dedup-fused-exact-pair-long-v4-20261014.json';assert not O.exists();probe=F/'probe.rs';probe.write_text('''use std::{path::Path, hint::black_box};
fn main(){let a:Vec<String>=std::env::args().collect();let reps:usize=a[3].parse().unwrap();let expected:bool=a[4].parse().unwrap();let mut supported=0usize;for _ in 0..reps {let value=black_box(rrrah_dedup::exact::compare_files(Path::new(&a[1]),Path::new(&a[2]),u64::MAX,||false).unwrap());assert_eq!(value,expected);supported+=usize::from(value);}println!("{{\\"calls\\":{},\\"supported\\":{}}}",reps,supported);}
''');env=os.environ.copy();env['CARGO_MANIFEST_DIR']=str(R/'crates/rrrah-dedup');d=dict(status='building',producer_pid=os.getpid(),producer_sha256=sha(__file__),gate_sha256=sha(G),probe_sha256=sha(probe),results=[],builds=[],scope='Actual public compare_files over synthetic stable regular files, nofeature isolated current-root baseline versus fusedcandidate, opt-level1 realdeps. Alternating fresh processes, childCPU from RUSAGE_CHILDREN deltas, wall recorded separately. No physicaldeviceIO measurement, coldcache/Windows/Linux/release/collection/decode/native/corpus/allcase or hardRSS claim. Production unchanged.')
def save():
 t=O.with_suffix('.tmp');t.write_text(json.dumps(d,indent=2)+'\n');t.replace(O)
save()
parent_path=D/'dedup-fused-exact-pair-benchmark-v3-20261014.json';parent=json.loads(parent_path.read_text());parent_audit=D/'dedup-fused-exact-pair-benchmark-v3-audit-20261014.json';assert json.loads(parent_audit.read_text())['processes']==126
oldF=Path('/private/tmp/rrrah-fused-exact-pair-benchmark-v3-20261014');oldK=D/'dedup-fused-exact-pair-benchmark-v3-20261014'
import shutil
for v in ['baseline','candidate']:
 V=F/v;V.mkdir();original=oldF/v/'probe';pin=next(x['executable_sha256']for x in parent['builds']if x['version']==v and x['label']=='probe');assert sha(original)==pin;subprocess.run(['cp','-cp',str(original),str(V/'probe')],check=True);assert sha(V/'probe')==pin
 for label in ['library','probe']:
  log=f'{v}-{label}-build.log';shutil.copyfile(oldK/log,K/log);assert sha(K/log)==next(x['log_sha256']for x in parent['builds']if x['version']==v and x['label']==label)
d.update(builds=parent['builds'],reused_from=str(parent_path.relative_to(R)),reused_from_sha256=sha(parent_path),reused_from_audit_sha256=sha(parent_audit),scope=d['scope']+' Reuses exact retained v3executables and originalbuildlogs; no recompilation. Only5ambiguouslarger cases repeat at16xmorecalls perprocess; empty/tiny robust cases are not repeated. No codechange or claimednewvariant.');save()
inputs=F/'inputs';inputs.mkdir();cases=[]
for label,length,reps,kind in [('block',65536,4096,'same'),('large_same',2*1024*1024,256,'same'),('large_early_different',2*1024*1024,256,'early'),('large_late_different',2*1024*1024,256,'late'),('large_size_different',2*1024*1024,256,'size')]:
 left=inputs/(label+'-left');right=inputs/(label+'-right');data=bytes(range(256))*(length//256)+bytes(range(length%256));other=bytearray(data)
 if kind=='early':other[0]^=1
 if kind=='late':other[-1]^=1
 if kind=='size':other.append(1)
 left.write_bytes(data);right.write_bytes(other);cases.append(dict(label=label,left=str(left),right=str(right),reps=reps,expected=kind=='same',input_sha256=[sha(left),sha(right)]))
d['cases']=cases;d.update(status='benchmarking');save()
for case in cases:
 for pair in range(9):
  for version in (['baseline','candidate']if pair%2==0 else['candidate','baseline']):
   command=[str(F/version/'probe'),case['left'],case['right'],str(case['reps']),str(case['expected']).lower()];before=resource.getrusage(resource.RUSAGE_CHILDREN);start=time.monotonic();c=subprocess.Popen(command,cwd=R,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);d.update(active_case=[case['label'],pair,version],child_pid=c.pid);save();stdout,stderr=c.communicate();wall=time.monotonic()-start;after=resource.getrusage(resource.RUSAGE_CHILDREN);cpu=(after.ru_utime-before.ru_utime)+(after.ru_stime-before.ru_stime);assert c.returncode==0 and stderr=='';value=json.loads(stdout);assert value==dict(calls=case['reps'],supported=case['reps']if case['expected']else 0)
   raw=K/f"{case['label']}-{pair}-{version}.json";raw.write_text(json.dumps(dict(command=command,returncode=c.returncode,stdout=stdout,stderr=stderr,child_cpu_seconds=cpu,wall_seconds=wall,usage_before=dict(user_seconds=before.ru_utime,system_seconds=before.ru_stime),usage_after=dict(user_seconds=after.ru_utime,system_seconds=after.ru_stime)),indent=2)+'\n');d['results'].append(dict(case=case['label'],pair=pair,version=version,warmup=pair==0,raw=str(raw.relative_to(R)),raw_sha256=sha(raw),cpu_seconds=cpu));save()
assert all(sha(k)==h for k,h in prior['dependency_pins'].items())and all(sha(S/'baseline'/k)==h for k,h in plan['variants']['baseline'].items())and all(sha(Path(g['source'])/k)==h for k,h in g['source_sha256'].items());assert all([sha(c['left']),sha(c['right'])]==c['input_sha256']for c in cases);summary=[]
for c in cases:
 ratios=[]
 for pair in range(1,9):
  values={r['version']:r['cpu_seconds']for r in d['results']if r['case']==c['label']and r['pair']==pair};assert set(values)=={'baseline','candidate'}and values['baseline']>0;ratios.append(values['candidate']/values['baseline'])
 summary.append(dict(case=c['label'],median_candidate_cpu_ratio=statistics.median(ratios),paired_ratios=ratios))
d.update(status='completed_pending_independent_audit',summary=summary,dependency_pins=prior['dependency_pins']);save();print(summary)
