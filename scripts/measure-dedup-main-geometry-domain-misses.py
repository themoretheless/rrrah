"""Exhaustive domain-eligible native geometry on the five frozen sparse misses."""
import hashlib,json,subprocess
from pathlib import Path
from dedup_jpeg_domain import oriented_dimensions
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');b=Path('docs/research');prepared=root/'prepared.json';m=json.loads(prepared.read_text());baseline=b/'dedup-combined-domain-release-original-full.json';r=json.loads(baseline.read_text());previous=b/'dedup-main-geometry-miss-exhaustive.json';old=json.loads(previous.read_text());exe=root/'geometry-domain-miss-probe-qualified';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
files=[exe,prepared,baseline,previous,Path(__file__),Path('crates/rrrah-dedup/src/geometry.rs'),Path('crates/rrrah-dedup/examples/geometry_domain_miss_probe.rs')];inputs=[]
for v in old['results']:
 name=v['query'];row=next(q for q in r['results'] if q['query']==name);points=b/f'dedup-main-geometry-miss-{name[:-4]}-points.txt';files.append(points);dims=[]
 for split,folder,filename in [('original','original-resolution-negative-200201',row['original']),('strong','original-resolution-strong-all',name)]:
  image=root/folder/filename;record=next(q for q in m['images'][split] if q['filename']==filename);assert h(image)==record['source_sha256'];files.append(image);dims.extend(oriented_dimensions(image,record['source_size']))
 inputs.append((name,points,dims))
pins={str(p):h(p) for p in files};output=b/'dedup-main-geometry-domain-miss-exhaustive.json';assert not output.exists();rows=[]
for name,points,dims in inputs:
 assert all(h(k)==v for k,v in pins.items())
 run=subprocess.run([str(exe),str(points),*map(str,dims)],capture_output=True,text=True);assert run.returncode==0,run.stderr;e=json.loads(run.stdout);rows.append({'query':name,'dimensions':dims,'returncode':0,'evidence':e})
assert all(h(k)==v for k,v in pins.items())
output.write_text(json.dumps({'status':'complete_native_exhaustive_domain_geometry_misses','input_hashes':pins,'results':rows,'scope':'Same sparse supplied points, exhaustive four-point hypotheses with full-domain eligibility before ranking/refinement, min10 target tolerance2. No descriptor retrieval, pixels/copy admission or independent no-model numerical oracle.'},indent=2)+'\n')
print([(v['query'],v['evidence']['status']) for v in rows])
