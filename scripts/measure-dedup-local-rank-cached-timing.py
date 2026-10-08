"""Alternating end-to-end normalized-pixel process measurements; no broad speed claim."""
import hashlib,json,statistics,subprocess,time
from pathlib import Path
D=Path('docs/research');T=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
executables={'direct':T/'local-rank-resolution-probe-qualified','cached':T/'local-rank-cached-pixels-probe-qualified'}
args=[T/'rank-normalized-pixel-oracle/200500.rgba32',T/'rank-normalized-pixel-oracle/200501.rgba32',T/'all-screen-local-rank-58.txt',T/'screen-local-rank-api-points.txt'];reference=D/'dedup-main-screen-local-rank-api.jsonl';expected=[json.loads(x) for x in reference.read_text().splitlines()];files=[*executables.values(),*args,reference,Path(__file__)];pins={str(p):h(p) for p in files};output=D/'dedup-local-rank-cached-process-timing.json';assert not output.exists();rows=[]
for iteration in range(6):
 for backend in (['direct','cached'] if iteration%2==0 else ['cached','direct']):
  start=time.perf_counter();p=subprocess.run([str(executables[backend]),*map(str,args)],capture_output=True,text=True);elapsed=time.perf_counter()-start;assert p.returncode==0,p.stderr;assert [json.loads(x) for x in p.stdout.splitlines()]==expected
  rows.append({'iteration':iteration,'backend':backend,'elapsed_seconds':elapsed,'returncode':p.returncode});print(iteration,backend,elapsed,flush=True)
assert all(h(p)==v for p,v in pins.items());summary={key:{'median_seconds':statistics.median(x['elapsed_seconds'] for x in rows if x['backend']==key),'min_seconds':min(x['elapsed_seconds'] for x in rows if x['backend']==key),'max_seconds':max(x['elapsed_seconds'] for x in rows if x['backend']==key)} for key in executables}
output.write_text(json.dumps({'status':'measured_alternating_exact_output_process_timings','rows':rows,'summary':summary,'input_hashes':pins,'scope':'Single selected real screen ROI, frozen release binaries, three filter levels/two directions plus normalized-pixel loading and snapshot hashing in each process. Six alternating measurements per backend under live concurrent workloads. No isolated kernel/decoder/retrieval/whole-collection performance claim; bytes and decisions exactly equal.'},indent=2)+'\n')
