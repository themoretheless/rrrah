"""All229 strong real pairs through fresh native automatic anchor file API."""
import hashlib,json,math,subprocess
from pathlib import Path
from dedup_jpeg_domain import oriented_dimensions
D=Path('docs/research');T=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');exe=T/'anchor-candidate-files-probe-qualified';baseline=D/'dedup-combined-domain-release-original-full.json';manifest=T/'prepared.json';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
b=json.loads(baseline.read_text());m=json.loads(manifest.read_text());records={x['filename']:x for v in m['images'].values() for x in v};pairs=b['results'];assert len(pairs)==229
priority=['202502.jpg','212602.jpg','204702.jpg','200501.jpg'];pairs=sorted(pairs,key=lambda r:(priority.index(r['query']) if r['query'] in priority else len(priority),r['query']))
out=D/'dedup-anchor-automatic-full.json';assert not out.exists();files=[exe,baseline,manifest,D/"dedup-anchor-automatic-full-source.json",Path(__file__),Path('scripts/dedup_jpeg_domain.py')]
for row in pairs:files.extend([T/'original-resolution-negative-200201'/row['original'],T/'original-resolution-strong-all'/row['query']])
files=list(dict.fromkeys(files));pins={str(p):h(p) for p in files};r={'status':'running','required_pairs':229,'results':[],'pins':pins,'scope':'Fresh native candidate union and guarded original-file anchors, all original union points preserved. Explicit source tolerance2*oriented diagonal ratio, target2/filter8/rank8/.005/1000/.3/.9. Separate experiment, not default promotion or unrelated precision/collection qualification.'}
def save():
 tmp=out.with_suffix('.tmp');tmp.write_text(json.dumps(r,indent=2)+'\n');tmp.replace(out)
save()
for row in pairs:
 source=T/'original-resolution-negative-200201'/row['original'];target=T/'original-resolution-strong-all'/row['query'];assert all(h(p)==pins[str(p)] for p in [source,target,exe]);sd=oriented_dimensions(source,records[row['original']]['source_size']);td=oriented_dimensions(target,records[row['query']]['source_size']);tol=2*math.hypot(*sd)/math.hypot(*td)
 p=subprocess.run([str(exe),str(source),str(target),str(tol)],capture_output=True,text=True);result={k:row[k] for k in ['original','query','group_id']};result.update({'source_tolerance':tol,'returncode':p.returncode,'stdout':p.stdout,'stderr':p.stderr})
 if p.returncode==0:result['evidence']=json.loads(p.stdout)
 assert all(h(p)==pins[str(p)] for p in [source,target,exe]);r['results'].append(result);save()
assert all(h(p)==digest for p,digest in ((Path(k),v) for k,v in pins.items()));r['status']='complete';save();print('Completed229 automatic anchor pairs')
