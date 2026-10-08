"""Frozen six-source collection versus every fresh pair; checkpoint each result."""
import hashlib,itertools,json,subprocess
from pathlib import Path
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
exe=root/'gradient-scales-probe-fresh-union-six-collection'
folder=root/'original-resolution-hard-crops'
names=['200100.jpg','200101.jpg','200200.jpg','200201.jpg','200300.jpg','200301.jpg']
out=Path('docs/research/dedup-fresh-union-six-collection-retry.json')
assert not out.exists()
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
pins={str(p):h(p) for p in [exe,Path(__file__),Path('docs/research/dedup-original-resolution-hard-crops.json'),*[folder/n for n in names]]}
r={'status':'running_collection','input_hashes':pins,'files':names,'results':[],'scope':'Six original-resolution sources; all 15 fresh pair comparisons against indexed collection with explicit two-recipe candidate union and unchanged grid8/radius7 pixel policy. Finite publisher-origin labels, not semantic precision.'}
def check():
    assert all(h(p)==v for p,v in pins.items()),'source drift'
def save():
    temp=out.with_suffix('.tmp');temp.write_text(json.dumps(r,indent=2)+'\n');temp.replace(out)
def run(args):
    check();v=subprocess.run([str(exe),*args],capture_output=True,text=True);check()
    if v.returncode:
        r.update(status='native_failed',returncode=v.returncode,stdout=v.stdout,stderr=v.stderr);save();raise SystemExit(v.returncode)
    e=json.loads(v.stdout);assert e['status']=='ok' and e['managed_used']==0 and e['managed_peak']<=512*1024*1024
    r.setdefault('native_outputs',[]).append({'args':args,'stdout':v.stdout,'stderr':v.stderr,'returncode':v.returncode})
    return e
save();r['collection']=run(['original-union-collection-six',str(folder),'512']);r['status']='running_fresh_pairs';save()
indexed={(p['left'],p['right']):p for p in r['collection']['pairs']}
keys=['correspondences','matrix','inliers','regions','region_support_count']
canonical=lambda e:json.dumps({k:e[k] for k in keys},sort_keys=True,allow_nan=False)
for i,j in itertools.combinations(range(6),2):
    e=run(['original-managed-candidate-union',str(folder/names[i]),str(folder/names[j])]);pair=indexed.get((i+1,j+1));accepted=e['region_support_count']>0
    row={'left':i+1,'right':j+1,'same_origin':i//2==j//2,'retrieved':pair is not None,'evidence':e};r['results'].append(row);save()
    assert pair is None or canonical(pair)==canonical(e),'collection/file evidence mismatch'
    assert not accepted or pair is not None,'accepted fresh pair omitted by retrieval'
    if accepted and i//2!=j//2:
        r['status']='false_positive_observed';save();raise SystemExit(2)
check();r['status']='complete_native_pair_parity';save()
