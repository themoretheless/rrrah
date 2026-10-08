"""Two different-origin native controls for the newly recovered fine-grid crop."""
import hashlib,json,subprocess
from pathlib import Path
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
exe=root/'gradient-scales-probe-original-area-grid8-blur7'
query=root/'original-resolution-hard-crops/200101.jpg'
origins=[root/'original-resolution-hard-crops'/f'{n}.jpg' for n in ['200200','200300']]
baseline=Path('docs/research/dedup-original-area-grid8-blur7-200101.json')
manifest=root/'prepared.json';out=Path('docs/research/dedup-area-grid8-negative-controls.json');assert not out.exists()
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
pins={str(p):h(p) for p in [exe,query,baseline,manifest,Path(__file__),*origins]}
metadata=json.loads(manifest.read_text());q=next(v for v in metadata['images']['strong'] if v['filename']==query.name)
assert h(query)==q['source_sha256']
r={'status':'running','input_hashes':pins,'query_group':q['group_id'],'results':[],'scope':'Two source-resolution different publisher-origin controls for area/grid8/radius7 candidate recipe; finite precision check, not all156 negatives or semantic/burst qualification.'}
save=lambda:out.write_text(json.dumps(r,indent=2)+'\n')
save()
for left in origins:
 entry=next(v for v in metadata['images']['original'] if v['filename']==left.name)
 assert entry['group_id']!=q['group_id'] and h(left)==entry['source_sha256']
 assert all(h(k)==v for k,v in pins.items())
 run=subprocess.run([str(exe),'original-managed-regions-area-grid8-blur7',str(left),str(query)],capture_output=True,text=True)
 row={'original':left.name,'original_group':entry['group_id'],'returncode':run.returncode,'stdout':run.stdout,'stderr':run.stderr};r['results'].append(row);save()
 if run.returncode:r['status']='native_failed';save();raise SystemExit(run.returncode)
 e=json.loads(run.stdout);row['evidence']=e;save()
 assert e['status']=='ok' and type(e['managed_used']) is int and e['managed_used']==0
 assert all(h(k)==v for k,v in pins.items())
 if e['whole_candidate'] or e['region_support_count']:
  r['status']='false_positive_observed';save();raise SystemExit(2)
 save()
r['status']='complete';save()
