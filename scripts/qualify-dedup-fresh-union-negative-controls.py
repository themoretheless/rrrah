"""Same frozen fresh compound recipe, two different-origin controls for known recovery."""
import hashlib,json,subprocess
from pathlib import Path
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');exe=root/'gradient-scales-probe-fresh-candidate-union';folder=root/'original-resolution-hard-crops';query=folder/'200301.jpg';baseline=Path('docs/research/dedup-fresh-candidate-union-200301.json');audit=Path('docs/research/dedup-fresh-candidate-union-200301-audit.json');out=Path('docs/research/dedup-fresh-union-negative-controls.json');assert not out.exists();h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
for p in [baseline,audit]:
 r=json.loads(p.read_text());assert all(h(k)==v for k,v in r['input_hashes'].items())
originals=[folder/'200100.jpg',folder/'200200.jpg'];pins={str(p):h(p) for p in [exe,query,*originals,baseline,audit,Path(__file__)]}
r={'status':'running_negatives','input_hashes':pins,'query':'200301.jpg','query_group':200300,'required_negatives':2,'results':[],'scope':'Two explicit other publisher-origin original JPEGs through the same frozen fresh union file API that recovered200301. Finite controls, not broad/semantic precision.'}
def save():
 t=out.with_suffix('.tmp');t.write_text(json.dumps(r,indent=2)+'\n');t.replace(out)
save()
for source in originals:
 assert all(h(k)==v for k,v in pins.items());v=subprocess.run([str(exe),'original-managed-candidate-union',str(source),str(query)],capture_output=True,text=True);assert all(h(k)==v for k,v in pins.items());row={'original':source.name,'original_group':int(source.stem),'returncode':v.returncode,'stdout':v.stdout,'stderr':v.stderr};r['results'].append(row);save()
 if v.returncode:r['status']='native_failed';save();raise SystemExit(v.returncode)
 e=json.loads(v.stdout);row['evidence']=e;save();assert e['status']=='ok' and e['managed_used']==0 and e['managed_peak']<=512*1024*1024
 if e['region_support_count']>0:r['status']='false_positive_observed';save();raise SystemExit(2)
r['status']='complete';save()
