"""Original-byte negative gate; first require positive output parity."""
import hashlib,json,subprocess,tarfile
from pathlib import Path
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
exe=root/'gradient-scales-probe-original-expanded'
manifest=root/'prepared.json';baseline=Path('docs/research/dedup-original-resolution-three-crops.json')
out=Path('docs/research/dedup-original-resolution-negatives-200201.json')
assert not out.exists()
h=lambda f:hashlib.sha256(Path(f).read_bytes()).hexdigest()
m=json.loads(manifest.read_text());originals=m['images']['original'];assert len(originals)==157
query=next(v for v in m['images']['strong'] if v['filename']=='200201.jpg')
assert query['group_id']==200200
archive=root/'copydays_original.tar.gz';archive_hash=h(archive)
directory=root/'original-resolution-negative-200201';assert not directory.exists();directory.mkdir()
pins={str(f):h(f) for f in [exe,manifest,baseline,archive,Path(__file__)]}
files={}
with tarfile.open(archive) as t:
 for source in originals:
  assert source['source_archive_sha256']==archive_hash
  assert Path(source['filename']).name==source['filename']
  data=t.extractfile(source['source_member']).read();assert hashlib.sha256(data).hexdigest()==source['source_sha256']
  f=directory/source['filename'];f.write_bytes(data);f.chmod(0o444);pins[str(f)]=h(f);files[source['filename']]=f
q=root/'original-resolution-hard-crops/200201.jpg';assert h(q)==query['source_sha256'];pins[str(q)]=h(q)
negative=[v for v in originals if v['group_id']!=query['group_id']];assert len(negative)==156
r={'status':'running_positive_parity','input_hashes':pins,'required_negatives':[v['filename'] for v in negative],'positive_control':None,'results':[], 'scope':'Original JPEG publisher-origin negatives; expanded resource limits only, exact positive native-output parity first. Plain diagnostic scratch excluded from managed claim; not semantic/burst or all-query precision.'}
def save():
 temporary=out.with_suffix('.tmp');temporary.write_text(json.dumps(r,indent=2)+'\n');temporary.replace(out)
def run(left):
 process=subprocess.run([str(exe),'scale-distinct-original',str(left),str(q)],capture_output=True,text=True)
 row={'left':str(left),'query':'200201.jpg','returncode':process.returncode,'stdout':process.stdout,'stderr':process.stderr}
 if process.returncode:return row
 e=json.loads(process.stdout);assert e['status']=='ok' and type(e['managed_used']) is int and e['managed_used']==0
 assert type(e['managed_peak']) is int and e['managed_peak']<=536870912
 row['evidence']=e
 assert all(h(k)==v for k,v in pins.items())
 return row
save();control=run(files['200200.jpg']);r['positive_control']=control;save()
if control['returncode']:r['status']='positive_native_failed';save();raise SystemExit(control['returncode'])
reference=next(v['evidence'] for v in json.loads(baseline.read_text())['results'] if v['query_id']=='200201.jpg')
canonical=lambda e:json.dumps({k:v for k,v in e.items() if k!='managed_peak'},sort_keys=True,allow_nan=False)
if canonical(control['evidence'])!=canonical(reference):r['status']='positive_output_parity_failed';save();raise SystemExit(1)
r['status']='running_negatives';save()
for source in negative:
 row=run(files[source['filename']]);row['publisher_group_id']=source['group_id'];r['results'].append(row);save()
 if row['returncode']:r['status']='negative_native_failed';save();raise SystemExit(row['returncode'])
 e=row['evidence'];flagged=any(v['distinct']['region_support_count'] or any(s['region_support_count'] for s in v['distinct']['similarity_candidates']) for v in e['stats'])
 if flagged:r['status']='negative_flagged';save();raise SystemExit(1)
r['status']='complete';save()
