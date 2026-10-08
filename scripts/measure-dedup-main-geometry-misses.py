"""Exhaustive native hypotheses on all five sparse geometry misses.
Supplied-point geometric diagnostic, not a file/copy acceptance test.
"""
import hashlib,json,subprocess
from pathlib import Path
b=Path('docs/research');baseline=b/'dedup-combined-domain-release-original-full.json';analysis=b/'dedup-main-problem-refusal-analysis.json';r=json.loads(baseline.read_text());a=json.loads(analysis.read_text());names=[v['query'] for v in a['rows'] if v['reason']=='geometry'];assert len(names)==5
exe=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays/geometry-miss-probe-qualified')
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
files=[exe,baseline,analysis,Path(__file__),Path('crates/rrrah-dedup/src/geometry.rs'),Path('crates/rrrah-dedup/examples/geometry_miss_probe.rs')]
inputs=[]
for name in names:
 e=next(v['evidence'] for v in r['results'] if v['query']==name);points=e['correspondences'];assert 4<=len(points)<=42
 path=b/f'dedup-main-geometry-miss-{name[:-4]}-points.txt';assert not path.exists();path.write_text(''.join(' '.join(repr(x) for pair in p for x in pair)+'\n' for p in points));files.append(path);inputs.append((name,path,points))
pins={str(p):h(p) for p in files};report={'status':'running','input_hashes':pins,'results':[],'scope':'Exhaustive native 4-point models on fixed sparse correspondences. All hypotheses admitted, min10 and target tolerance2 unchanged. Does not enforce whole-image horizon constraints or prove pixels/copy identity.'};output=b/'dedup-main-geometry-miss-exhaustive.json';assert not output.exists()
def save():
 tmp=output.with_suffix('.tmp');tmp.write_text(json.dumps(report,indent=2)+'\n');tmp.replace(output)
save()
for name,path,points in inputs:
 assert all(h(p)==v for p,v in pins.items())
 p=subprocess.run([str(exe),str(path)],capture_output=True,text=True);assert p.returncode==0,p.stderr;e=json.loads(p.stdout)
 if e['status']=='model':
  assert len(e['inliers'])>=10
  for i in e['inliers']:
   (x,y),(tx,ty)=points[i];m=e['matrix'];z=m[2][0]*x+m[2][1]*y+m[2][2];assert z!=0;assert ((m[0][0]*x+m[0][1]*y+m[0][2])/z-tx)**2+((m[1][0]*x+m[1][1]*y+m[1][2])/z-ty)**2<=4.0000001
 report['results'].append({'query':name,'returncode':0,'evidence':e});save()
assert all(h(p)==v for p,v in pins.items());report['status']='complete';save()
print([(v['query'],v['evidence']['status'],len(v['evidence'].get('inliers',[]))) for v in report['results']])
