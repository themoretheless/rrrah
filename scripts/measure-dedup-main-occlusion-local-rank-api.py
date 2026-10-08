"""Real integrated API, original points; selected regions retain global models."""
import hashlib,json,shutil,subprocess
from pathlib import Path
D=Path('docs/research');T=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
exe=T/'local-rank-pixels-probe-qualified';assert not exe.exists();shutil.copy2('target/release/examples/local_rank_pixels_probe',exe)
base=D/'dedup-main-photometric-misses-radius8.json';rank=D/'dedup-main-occlusion-all-rank-regions.json';b=json.loads(base.read_text());r=json.loads(rank.read_text());out=D/'dedup-main-occlusion-local-rank-api.json';assert not out.exists()
files=[base,rank,exe,Path(__file__),Path('crates/rrrah-dedup/src/local_rank.rs'),Path('crates/rrrah-dedup/examples/local_rank_pixels_probe.rs'),Path('crates/rrrah-dedup/Cargo.toml')]
report={'status':'running','results':[],'input_hashes':{str(p):h(p) for p in files},'scope':'Original complete spatially distinct correspondences, selected previously measured regions, supplied global model; explicit source/target tolerance2/min10 geometry plus both pixel directions. Local support only, not file classification or broad precision.'}
def save():out.write_text(json.dumps(report,indent=2)+'\n')
for query,index in [('214402.jpg',79),('214402.jpg',104),('214402.jpg',105),('212602.jpg',89),('212602.jpg',23)]:
 old=next(v for v in b['results'] if v['query']==query);e=old['evidence'];row=next(x for x in r['results'] if x['query']==query and x['region_index']==index)
 points=T/f'local-rank-api-{Path(query).stem}-points.txt'
 if not points.exists():points.write_text('\n'.join(' '.join(map(str,p[0]+p[1])) for p in e['correspondences'])+'\n')
 pixels=[T/'rank-normalized-pixel-oracle'/('occlusion-'+Path(name).stem+'.rgba32') for name in [old['original'],query]];model=T/f'occlusion-all-rank-{Path(query).stem}-{index}.txt'
 for p in [points,model,*pixels]:report['input_hashes'][str(p)]=h(p)
 run=subprocess.run([str(exe),*map(str,pixels),str(model),str(points)],capture_output=True,text=True)
 result={'query':query,'region_index':index,'domains':row['domains'],'returncode':run.returncode,'stdout':run.stdout,'stderr':run.stderr}
 if run.returncode==0:result['evidence']=[json.loads(line) for line in run.stdout.splitlines()]
 report['results'].append(result);save()
assert all(h(p)==v for p,v in report['input_hashes'].items());report['status']='complete';save()
