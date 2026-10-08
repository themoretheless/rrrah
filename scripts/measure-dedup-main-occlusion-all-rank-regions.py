"""All existing regions, fixed geometry; no rank-only admission."""
import hashlib,json,subprocess
from pathlib import Path
D=Path('docs/research');T=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
base=D/'dedup-main-photometric-misses-radius8.json';b=json.loads(base.read_text());exe=T/'projective-rank-regions-probe-qualified'
out=D/'dedup-main-occlusion-all-rank-regions.json';assert not out.exists()
r={'status':'running','required_regions':216,'results':[],'input_hashes':{str(p):h(p) for p in [base,exe,Path(__file__)]},'scope':'All108 existing regions per pair, fixed geometry/filter0,3,8. Rank-only diagnostic is not copy admission; broad unrelated calibration and independent pixel verification pending.'}
def save():
 tmp=out.with_suffix('.tmp');tmp.write_text(json.dumps(r,indent=2)+'\n');tmp.replace(out)
for original,query in [('214400','214402'),('212600','212602')]:
 e=next(v['evidence'] for v in b['results'] if v['query']==query+'.jpg');assert len(e['regions'])==108
 pixels=[T/'rank-normalized-pixel-oracle'/('occlusion-'+x+'.rgba32') for x in [original,query]]
 for p in pixels:r['input_hashes'][str(p)]=h(p)
 save()
 for index,region in enumerate(e['regions']):
  model=T/f'occlusion-all-rank-{query}-{index}.txt';assert not model.exists()
  values=[x for row in e['matrix'] for x in row]+[x for rect in region['domains'] for x in rect]
  model.write_text(' '.join(map(str,values))+'\n');r['input_hashes'][str(model)]=h(model)
  p=subprocess.run([str(exe),*map(str,pixels),str(model)],capture_output=True,text=True)
  row={'original':original+'.jpg','query':query+'.jpg','region_index':index,'domains':region['domains'],'returncode':p.returncode,'stdout':p.stdout,'stderr':p.stderr}
  if p.returncode==0:row['rank']=[json.loads(line) for line in p.stdout.splitlines()]
  r['results'].append(row);save()
assert all(h(p)==v for p,v in r['input_hashes'].items());r['status']='complete';save()
