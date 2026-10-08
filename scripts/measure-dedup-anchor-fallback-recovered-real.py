"""Three new frozen fallback-supported real pairs through actual guarded anchor API."""
import hashlib,json,math,subprocess
from pathlib import Path
from dedup_jpeg_domain import oriented_dimensions
D=Path('docs/research');T=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');exe=T/'anchor-fallback-files-probe-qualified';manifest=T/'prepared.json';base=D/'dedup-fallback-release-original-full.json';m=json.loads(manifest.read_text());b=json.loads(base.read_text());records={x['filename']:x for v in m['images'].values() for x in v};queries=['201303.jpg','207002.jpg','210202.jpg'];out=D/'dedup-anchor-fallback-recovered-real.json';assert not out.exists();h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();pairs=[next(x for x in b['results'] if x['query']==q) for q in queries];files=[exe,manifest,base,D/"dedup-anchor-fallback-real-source.json",Path(__file__),Path('scripts/dedup_jpeg_domain.py')]
for row in pairs:files.extend([T/'original-resolution-negative-200201'/row['original'],T/'original-resolution-strong-all'/row['query']])
files=list(dict.fromkeys(files));pins={str(p):h(p) for p in files};r={'status':'running','required_pairs':3,'pins':pins,'results':[],'scope':'Actual fresh guarded fallback geometry plus anchor files for three newly color-supported real pairs. Existing color filter7 fallback recipe, ordinal filter8/rank8/contrast.005/min1000/.3/.9, min10 geometry/anchors and target2/source2diagonal ratio unchanged. Not full corpus, precision or rank-driven fallback.'}
def save():
 tmp=out.with_suffix('.tmp');tmp.write_text(json.dumps(r,indent=2)+'\n');tmp.replace(out)
save()
for row in pairs:
 source=T/'original-resolution-negative-200201'/row['original'];target=T/'original-resolution-strong-all'/row['query'];sd=oriented_dimensions(source,records[row['original']]['source_size']);td=oriented_dimensions(target,records[row['query']]['source_size']);tol=2*math.hypot(*sd)/math.hypot(*td);p=subprocess.run([str(exe),str(source),str(target),str(tol)],capture_output=True,text=True);result={'original':row['original'],'query':row['query'],'source_tolerance':tol,'returncode':p.returncode,'stdout':p.stdout,'stderr':p.stderr}
 if p.returncode==0:result['evidence']=json.loads(p.stdout)
 assert all(h(k)==v for k,v in pins.items());r['results'].append(result);save()
r['status']='complete';save();print([(x['query'],x.get('evidence',{}).get('status'),x.get('evidence',{}).get('supported')) for x in r['results']])
