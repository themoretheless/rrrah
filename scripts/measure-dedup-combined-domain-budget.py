"""Fresh repeats with an explicit combined-domain pixel allowance and a known-positive parity control."""
import hashlib,json,subprocess
from pathlib import Path
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');exe=root/'gradient-scales-probe-combined-domain-budget';manifest=root/'prepared.json';baseline=Path('docs/research/dedup-fresh-union-original-full-strict-checkpoint-12-screen-run.json');output=Path('docs/research/dedup-combined-domain-budget.json');assert not output.exists()
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();m=json.loads(manifest.read_text());rows=json.loads(baseline.read_text())['results'];cases=[]
for query in ['200301.jpg','200701.jpg','200801.jpg']:
 row=next(x for x in rows if x['query']==query);left=root/'original-resolution-negative-200201'/row['original'];right=root/'original-resolution-strong-all'/query
 for p,split in [(left,'original'),(right,'strong')]:assert h(p)==next(v['source_sha256'] for v in m['images'][split] if v['filename']==p.name)
 cases.append((row,left,right))
pins={str(p):h(p) for p in [exe,manifest,baseline,Path(__file__),Path('crates/rrrah-dedup/src/local_scan.rs')]+[p for row,left,right in cases for p in [left,right]]}
r={'status':'running_pairs','input_hashes':pins,'results':[],'required_pairs':[row['query'] for row,left,right in cases],'max_combined_domain_pixels':12800000,'scope':'Same frozen compound recipe/512MiB; early original decode release plus explicit12.8M combined-domain pixel admission. Known-positive canonical parity and budget-refusal repeats, no full229 replacement claim.'}
def save():
 t=output.with_suffix('.tmp');t.write_text(json.dumps(r,indent=2)+'\n');t.replace(output)
def check():assert all(h(k)==v for k,v in pins.items())
check();save()
for row,left,right in cases:
 check();v=subprocess.run([str(exe),'original-managed-candidate-union-domain-budget',str(left),str(right)],capture_output=True,text=True);check();entry={'query':right.name,'original':left.name,'returncode':v.returncode,'stdout':v.stdout,'stderr':v.stderr}
 if v.returncode==0:
  e=json.loads(v.stdout);entry['evidence']=e;assert e['managed_used']==0 and e['managed_peak']<=512*1024*1024
  if right.name=='200301.jpg':assert {k:x for k,x in e.items() if k!='managed_peak'}=={k:x for k,x in row['evidence'].items() if k!='managed_peak'}
 r['results'].append(entry);save()
check();r['status']='complete';save()
