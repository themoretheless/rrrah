"""Real anchor-only native file parity; observed whole-process wall times."""
import hashlib,json,subprocess,time
from pathlib import Path
D=Path('docs/research');T=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');exe=T/'anchor-geometry-files-probe-qualified';baseline=D/'dedup-anchor-automatic-pixels-prefix-4.json';old=json.loads(baseline.read_text());out=D/'dedup-anchor-geometry-real-parity.json';assert not out.exists();h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();files=[exe,baseline,D/'dedup-anchor-geometry-real-source.json',Path(__file__)];rows=old['results'][:4]
for r in rows:files.extend([T/'original-resolution-negative-200201'/r['original'],T/'original-resolution-strong-all'/r['query']])
files=list(dict.fromkeys(files));pins={str(p):h(p) for p in files};report={'status':'running','required_pairs':4,'results':[],'pins':pins,'scope':'Four real prioritized pairs, same frozen legacy anchor output including complete points/model/counts; new proposal+anchor-only pipeline omits legacy color diagnostics. Whole process including decoder/hash/extraction/rank, concurrent workloads; no comparative speed claim or broad precision.'}
def save():
 tmp=out.with_suffix('.tmp');tmp.write_text(json.dumps(report,indent=2)+'\n');tmp.replace(out)
save()
for r in rows:
 source=T/'original-resolution-negative-200201'/r['original'];target=T/'original-resolution-strong-all'/r['query'];before=time.perf_counter();p=subprocess.run([str(exe),str(source),str(target),str(r['source_tolerance'])],capture_output=True,text=True);elapsed=time.perf_counter()-before;assert p.returncode==0,p.stderr;e=json.loads(p.stdout);assert e==r['evidence'],r['query'];assert all(h(p)==pin for p,pin in ((Path(k),v) for k,v in pins.items()));report['results'].append({'query':r['query'],'original':r['original'],'source_tolerance':r['source_tolerance'],'seconds':elapsed,'returncode':p.returncode,'stderr':p.stderr,'evidence':e});save()
report['status']='complete';save();print([(r['query'],r['seconds']) for r in report['results']])
