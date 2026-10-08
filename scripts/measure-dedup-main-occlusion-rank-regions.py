"""Rank diagnostic on best measured bidirectional color region of two occluded copies."""
import hashlib,json,subprocess
from pathlib import Path
D=Path('docs/research'); T=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
baseline=D/'dedup-main-photometric-misses-radius8.json'; b=json.loads(baseline.read_text())
dump=T/'rank-pixel-dump';exe=T/'projective-rank-regions-probe-qualified'
report={'status':'running','results':[],'input_hashes':{str(p):h(p) for p in [baseline,dump,exe,Path(__file__)]},'scope':'Best preexisting color-support regions selected before rank measurement. Diagnostic only; rank alone can accept unrelated images and cannot establish copy identity.'}
out=D/'dedup-main-occlusion-rank-regions.json';assert not out.exists()
def save():out.write_text(json.dumps(report,indent=2)+'\n')
save()
for original,query in [('214400.jpg','214402.jpg'),('212600.jpg','212602.jpg')]:
 e=next(v['evidence'] for v in b['results'] if v['query']==query)
 eligible=[(i,r) for i,r in enumerate(e['regions']) if r.get('pixels') and all(c[1]>=1000 and c[1]/c[2]>=.3 for c in r['pixels']['counts'])]
 index,region=max(eligible,key=lambda item:min(c[0]/c[1] for c in item[1]['pixels']['counts']))
 pixels=[]
 for name,folder in [(original,'original-resolution-negative-200201'),(query,'original-resolution-strong-all')]:
  source=T/folder/name;target=T/'rank-normalized-pixel-oracle'/('occlusion-'+Path(name).stem+'.rgba32');assert not target.exists()
  report['input_hashes'][str(source)]=h(source)
  p=subprocess.run([str(dump),str(source),str(target)],capture_output=True,text=True);assert p.returncode==0,p.stderr
  assert json.loads(p.stdout)['managed_used']==0
  report['input_hashes'][str(target)]=h(target);pixels.append(target)
 model=T/('occlusion-rank-'+Path(query).stem+'.txt');assert not model.exists()
 values=[x for row in e['matrix'] for x in row]+[x for rect in region['domains'] for x in rect]
 model.write_text(' '.join(map(str,values))+'\n');report['input_hashes'][str(model)]=h(model)
 p=subprocess.run([str(exe),*map(str,pixels),str(model)],capture_output=True,text=True)
 row={'original':original,'query':query,'region_index':index,'domains':region['domains'],'baseline_color_counts':region['pixels']['counts'],'returncode':p.returncode,'stdout':p.stdout,'stderr':p.stderr}
 if p.returncode==0:row['rank']=[json.loads(line) for line in p.stdout.splitlines()]
 report['results'].append(row);save()
assert all(h(p)==v for p,v in report['input_hashes'].items())
report['status']='complete';save()
