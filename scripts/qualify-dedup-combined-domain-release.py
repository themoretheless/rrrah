"""Frozen updated recipe release parity: three positives and two negatives."""
import hashlib,json,subprocess
from pathlib import Path
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');exe=root/'gradient-scales-probe-combined-domain-release';output=Path('docs/research/dedup-combined-domain-release.json');assert not output.exists()
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
a=Path('docs/research/dedup-combined-domain-budget-checkpoint-2.json');b=Path('docs/research/dedup-early-decode-release.json');c=Path('docs/research/dedup-fresh-union-negative-controls.json');docs=[json.loads(p.read_text()) for p in [a,b,c]]
assert docs[1]['status']=='complete' and docs[2]['status']=='complete'
cases=[(v['original'],v['query'],v['evidence']) for v in docs[0]['results']];v=next(v for v in docs[1]['results'] if v['query']=='200801.jpg');cases.append((v['original'],v['query'],v['evidence']));cases.extend((v['original'],'200301.jpg',v['evidence']) for v in docs[2]['results']);assert len(cases)==5
inputs=[exe,a,b,c,root/'prepared.json',Path(__file__)]
for l,r,e in cases:inputs.extend([root/'original-resolution-negative-200201'/l,root/'original-resolution-strong-all'/r])
pins={str(p):h(p) for p in inputs};r={'status':'running','input_hashes':pins,'required_pairs':5,'results':[],'scope':'Updated early-release plus explicit12.8M combined-domain policy; three positive/two negative debug-release canonical parity, same512MiB/acceptance. No full229 release recall or controlled speed claim.'}
def save():
 t=output.with_suffix('.tmp');t.write_text(json.dumps(r,indent=2)+'\n');t.replace(output)
canonical=lambda e:{k:v for k,v in e.items() if k!='managed_peak'}
def check():assert all(h(k)==v for k,v in pins.items())
check();save()
for l,q,reference in cases:
 check();v=subprocess.run([str(exe),'original-managed-candidate-union-domain-budget',str(root/'original-resolution-negative-200201'/l),str(root/'original-resolution-strong-all'/q)],capture_output=True,text=True);check();row={'original':l,'query':q,'returncode':v.returncode,'stdout':v.stdout,'stderr':v.stderr};r['results'].append(row);save()
 if v.returncode:r['status']='native_failed';save();raise SystemExit(v.returncode)
 e=json.loads(v.stdout);row['evidence']=e;assert canonical(e)==canonical(reference) and e['managed_used']==0 and e['managed_peak']<=512*1024*1024;save()
r['status']='verified_native_release_parity';save()
