import json,hashlib,subprocess,shutil
from pathlib import Path
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
exe=root/'gradient-scales-probe-fallback';assert not exe.exists();shutil.copyfile('target/debug/examples/gradient_scales_probe',exe);exe.chmod(0o555)
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
pairs=[('200300.jpg','200301.jpg'),('201300.jpg','201303.jpg')]
paths=[exe,Path(__file__),Path('crates/rrrah-dedup/src/local_scan.rs'),Path('crates/rrrah-dedup/examples/gradient_scales_probe.rs')]
for a,b in pairs:paths += [root/'original-resolution-negative-200201'/a,root/'original-resolution-strong-all'/b]
pins={str(p):h(p) for p in paths};report=Path('docs/research/dedup-fallback-known-pairs.json');assert not report.exists()
r={'status':'running_pairs','required_pairs':2,'input_hashes':pins,'results':[]}
def save():report.write_text(json.dumps(r,indent=2)+'\n')
def verify():assert all(h(p)==v for p,v in pins.items())
save()
for a,b in pairs:
 verify();argv=[str(exe),'original-managed-candidate-union-fallback',str(root/'original-resolution-negative-200201'/a),str(root/'original-resolution-strong-all'/b)]
 result=subprocess.run(argv,capture_output=True,text=True);verify();row={'original':a,'query':b,'argv':argv,'returncode':result.returncode,'stdout':result.stdout,'stderr':result.stderr}
 if result.returncode==0:row['evidence']=json.loads(result.stdout)
 r['results'].append(row);save()
r['status']='complete';verify();save()
