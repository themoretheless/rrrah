"""Fixed-threshold multi-partition geometry on dense wrinkled-print proposals."""
import hashlib,json,subprocess,shutil
from pathlib import Path
b=Path('docs/research');root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');exe=root/'partition-geometry-probe-wrinkled';assert exe.exists()
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();points=b/'dedup-wrinkled-dense-distinct-points.txt';prior=b/'dedup-wrinkled-dense-patches-bounded.json';audit=b/'dedup-wrinkled-dense-patches-bounded-audit.json';r=json.loads(prior.read_text());a=json.loads(audit.read_text());assert a['status']=='verified_dense_patch_proposal_arithmetic' and all(h(k)==v for obj in [r,a] for k,v in obj['input_hashes'].items());import math
selected=[]
for row in sorted(r['evidence']['matches'],key=lambda v:(v['squared_distance'],v['source'],v['target'])):
 if all(all(math.dist(row[k],prior[k])>2 for k in ['source','target']) for prior in selected):selected.append(row)
assert [[float(v) for v in line.split()] for line in points.read_text().splitlines()]==[row['source']+row['target'] for row in selected]
pins={str(p):h(p) for p in [exe,points,prior,audit,Path(__file__),Path('crates/rrrah-dedup/examples/partition_geometry_probe.rs')]};results=[]
for divisions in [1,2,4]:
 assert all(h(k)==v for k,v in pins.items());v=subprocess.run([str(exe),str(points),'800','600',str(divisions)],capture_output=True,text=True);assert v.returncode==0 and all(h(k)==v for k,v in pins.items());e=json.loads(v.stdout);assert e['status']=='ok' and len(e['regions'])==divisions*divisions;results.append({'divisions':divisions,'returncode':v.returncode,'stdout':v.stdout,'stderr':v.stderr,'evidence':e})
output=b/'dedup-wrinkled-dense-distinct-partitions.json';assert not output.exists();output.write_text(json.dumps({'status':'complete_native_experiment','input_hashes':pins,'results':results,'scope':'All image,2x2,4x4 native projective fits of the same deterministic distinct-location dense proposals. Fixed min10/tolerance2/4096 trials; no pixel acceptance or independent geometry completeness proof.'},indent=2)+'\n');print(json.dumps([{'divisions':v['divisions'],'models':sum(row['matrix'] is not None for row in v['evidence']['regions']),'point_counts':[len(row['point_indices']) for row in v['evidence']['regions']]} for v in results]))
