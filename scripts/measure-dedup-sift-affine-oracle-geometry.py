"""Native full-domain geometry of all four externally proposed SIFT cases."""
import hashlib,json,subprocess
from pathlib import Path
from dedup_jpeg_domain import oriented_dimensions
b=Path('docs/research');oracle=b/'dedup-sift-affine-oracle.json';o=json.loads(oracle.read_text());h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();assert o['status']=='complete_external_candidate_oracle' and len(o['results'])==4 and all(h(k)==v for k,v in o['input_hashes'].items());exe=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays/partition-geometry-probe-wrinkled');manifest=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays/prepared.json');m=json.loads(manifest.read_text());pins={str(p):h(p) for p in [oracle,exe,manifest,Path(__file__)]};results=[]
for row in o['results']:
 path=b/('dedup-sift-affine-'+row['case']+'-points.txt');assert not path.exists();path.write_text(''.join(' '.join(str(v) for v in p['source']+p['target'])+'\n' for p in row['distinct_matches']));pins[str(path)]=h(path)
 right=Path(row['right']);split='strong' if right.parent.name=='original-resolution-strong-all' else 'original';image=next(v for v in m['images'][split] if v['filename']==right.name);width,height=oriented_dimensions(right,image['source_size'])
 assert all(h(k)==v for k,v in pins.items());v=subprocess.run([str(exe),str(path),str(width),str(height),'1'],capture_output=True,text=True);assert all(h(k)==v for k,v in pins.items());result={'case':row['case'],'point_file':str(path),'target_dimensions':[width,height],'returncode':v.returncode,'stdout':v.stdout,'stderr':v.stderr}
 if v.returncode==0:result['evidence']=json.loads(v.stdout)
 results.append(result)
output=b/'dedup-sift-affine-oracle-geometry.json';assert not output.exists();output.write_text(json.dumps({'status':'complete_native_geometry_cases','input_hashes':pins,'required_cases':4,'results':results,'scope':'Native full-domain min10/tol2/4096 trials geometry on every distinct SIFT proposal case; errors remain denominator cases. No pixel acceptance, ground truth or library integration.'},indent=2)+'\n');print(json.dumps([{'case':v['case'],'returncode':v['returncode'],'models':sum(row['matrix'] is not None for row in v.get('evidence',{}).get('regions',[])),'inliers':[len(row['inliers']) for row in v.get('evidence',{}).get('regions',[])]} for v in results]))
