"""Resume the independently audited stopped negative prefix without modifying its provenance."""
import hashlib,json,subprocess
from pathlib import Path
h=lambda f:hashlib.sha256(Path(f).read_bytes()).hexdigest()
predecessor=Path('docs/research/dedup-fallback-release-original-negatives-201303.json')
audit=Path('docs/research/dedup-fallback-release-original-negatives-201303-stopped-prefix-audit.json')
a=json.loads(audit.read_text())
assert a['status']=='verified_fallback_release_negative_prefix' and a['verified_pairs']==1
assert all(h(k)==v for k,v in a['input_hashes'].items())
r=json.loads(predecessor.read_text())
assert len(r['results'])==1 and r['status']=='running_negatives'
assert all(h(k)==v for k,v in r['input_hashes'].items())
pins=dict(r['input_hashes'])
for path in [predecessor,audit,Path(__file__)]:pins[str(path)]=h(path)
r['input_hashes']=pins
r['resume_source']={'report':str(predecessor),'audit':str(audit),'verified_prefix':1,'reason':'Producer tuple-key error after saving first native result'}
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
exe=root/'gradient-scales-probe-fallback-release'
query=root/'original-resolution-strong-all/201303.jpg'
source_dir=root/'original-resolution-negative-200201'
m=json.loads((root/'prepared.json').read_text())
negatives=[v for v in m['images']['original'] if v['group_id']!=r['query_group']]
assert len(negatives)==156 and r['results'][0]['original']==negatives[0]['filename']
out=Path('docs/research/dedup-fallback-release-original-negatives-201303-resumed.json')
assert not out.exists()
def save():
 tmp=out.with_suffix('.tmp');tmp.write_text(json.dumps(r,indent=2)+'\n');tmp.replace(out)
def run(left):
 assert all(h(k)==v for k,v in pins.items())
 result=subprocess.run([str(exe),'original-managed-candidate-union-fallback',str(left),str(query)],capture_output=True,text=True)
 row={'returncode':result.returncode,'stdout':result.stdout,'stderr':result.stderr}
 assert all(h(k)==v for k,v in pins.items())
 if result.returncode:return row
 row['evidence']=json.loads(result.stdout);assert all(h(k)==v for k,v in pins.items())
 return row
save()
for source in negatives[len(r['results']):]:
 row=run(source_dir/source['filename']);row.update(original=source['filename'],original_group=source['group_id']);r['results'].append(row);save()
 if row['returncode']:r['status']='negative_native_failed';save();raise SystemExit(row['returncode'])
 e=row['evidence'];assert e['status']=='ok' and type(e['managed_used']) is int and e['managed_used']==0
 assert type(e['attempted_recipes']) is int and 1<=e['attempted_recipes']<=3
 assert e['smoothing_radii']==[[2,2],[4,0],[0,4]][e['attempted_recipes']-1]
 if e['region_support_count']:r['status']='false_positive_observed';save();raise SystemExit(2)
 assert e['attempted_recipes']==3
 save()
assert all(h(k)==v for k,v in pins.items());r['status']='complete';save()
