import hashlib,json,subprocess
from pathlib import Path
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
exe=root/'gradient-scales-probe-original-area-blur7'
left=root/'original-resolution-hard-crops/200100.jpg';right=root/'original-resolution-hard-crops/200101.jpg'
baseline=Path('docs/research/dedup-original-area-regions.json');out=Path('docs/research/dedup-original-area-blur7-200101.json');assert not out.exists()
h=lambda f:hashlib.sha256(Path(f).read_bytes()).hexdigest()
pins={str(f):h(f) for f in [exe,left,right,baseline,Path(__file__)]}
r={'status':'running','input_hashes':pins,'scope':'One original-resolution positive through the managed whole/regional file API; exact area-sampling geometry parity to radius3 diagnostic; radius7 pixel confirmation measured separately, no negative precision or allocation completeness claim.'}
def save():out.write_text(json.dumps(r,indent=2)+'\n')
save();run=subprocess.run([str(exe),'original-managed-regions-area-blur7',str(left),str(right)],capture_output=True,text=True)
r.update(returncode=run.returncode,stdout=run.stdout,stderr=run.stderr);save()
if run.returncode:r['status']='native_failed';save();raise SystemExit(run.returncode)
e=json.loads(run.stdout);assert e['status']=='ok' and type(e['managed_used']) is int and e['managed_used']==0
assert type(e['retained_before_drop']) is int and e['retained_before_drop']>0
reference=next(v['evidence'] for v in json.loads(baseline.read_text())['results'] if v['query_id']=='200101.jpg')
keys=['correspondences','matrix','inliers']
canonical=lambda value:json.dumps({k:value[k] for k in keys},sort_keys=True,allow_nan=False)
if canonical(e)!=canonical(reference):r['status']='native_output_parity_failed';r['evidence']=e;save();raise SystemExit(1)
assert all(h(k)==v for k,v in pins.items());r.update(status='verified_native_output_parity',evidence=e);save()
