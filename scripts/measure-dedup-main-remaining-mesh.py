"""Remaining paper/grille cases: retain all original points or whole prior inlier set."""
import hashlib,json,shutil,subprocess
from pathlib import Path
D=Path('docs/research');T=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();base=D/'dedup-combined-domain-release-original-full.json';b=json.loads(base.read_text());exe=T/'mesh-pixels-dimensions-probe-qualified';assert not exe.exists();shutil.copy2('target/release/examples/mesh_pixels_dimensions_probe',exe);work=T/'remaining-mesh';work.mkdir(exist_ok=False);inputs=T/'all-photometric-local-rank';out=D/'dedup-main-remaining-mesh.json';assert not out.exists();report={'status':'running','results':[],'required_cases':6,'input_hashes':{str(p):h(p) for p in [base,exe,Path(__file__),Path('crates/rrrah-dedup/examples/mesh_pixels_dimensions_probe.rs')]},'generated_hashes':{},'scope':'Three remaining paper/grille cases, complete original points or complete previous global inliers as explicit separate inputs. Target triangulation, bounded diagonal proposals, full source/target conforming mesh admission, whole-frame bidirectional rank. No post-refusal pruning, held-out calibration or copy admission.'}
for original,query in [('202500','202502'),('204700','204702'),('212600','212602')]:
 e=next(v['evidence'] for v in b['results'] if v['query']==query+'.jpg');pixels=[inputs/f'{name}.rgba32' for name in [original,query]]
 for p in pixels:report['input_hashes'][str(p)]=h(p)
 for kind,indices in [('all',list(range(len(e['correspondences'])))),('global_inliers',e['inliers'])]:
  points=work/f'{query}-{kind}-points.txt';points.write_text('\n'.join(' '.join(map(str,p[0]+p[1])) for p in [e['correspondences'][i] for i in indices])+'\n');report['generated_hashes'][str(points)]=h(points)
  run=subprocess.run([str(exe),*map(str,pixels),str(points)],capture_output=True,text=True);row={'query':query+'.jpg','point_set':kind,'point_indices':indices,'returncode':run.returncode,'stdout':run.stdout,'stderr':run.stderr}
  if run.returncode==0:row['rank']=[json.loads(x) for x in run.stdout.splitlines()]
  report['results'].append(row);out.write_text(json.dumps(report,indent=2)+'\n');print(query,kind,run.returncode,run.stderr.strip(),flush=True)
assert all(h(p)==v for p,v in report['input_hashes'].items()) and all(h(p)==v for p,v in report['generated_hashes'].items());report['status']='complete';out.write_text(json.dumps(report,indent=2)+'\n')
