"""Validate exact paired timing log/order, pins and summary without generalizing scope."""
import hashlib,json,math,statistics
from pathlib import Path
D=Path('docs/research');p=D/'dedup-local-rank-cached-process-timing.json';log=D/'dedup-local-rank-cached-process-timing.log';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();r=json.loads(p.read_text());assert r['status']=='measured_alternating_exact_output_process_timings';assert all(h(k)==v for k,v in r['input_hashes'].items());assert len(r['rows'])==12
expected=[(i,b) for i in range(6) for b in (['direct','cached'] if i%2==0 else ['cached','direct'])];assert [(x['iteration'],x['backend']) for x in r['rows']]==expected
lines=log.read_text().splitlines();assert len(lines)==12
for row,line in zip(r['rows'],lines):
 iteration,backend,value=line.split();assert int(iteration)==row['iteration'] and backend==row['backend'] and float(value)==row['elapsed_seconds'] and math.isfinite(float(value)) and float(value)>0 and row['returncode']==0
for key in ['direct','cached']:
 values=[x['elapsed_seconds'] for x in r['rows'] if x['backend']==key];assert r['summary'][key]=={'median_seconds':statistics.median(values),'min_seconds':min(values),'max_seconds':max(values)}
result={'status':'verified_alternating_selected_roi_process_timing','measurements':12,'summary':r['summary'],'observed_median_ratio':r['summary']['direct']['median_seconds']/r['summary']['cached']['median_seconds'],'pins':{str(x):h(x) for x in [p,log,Path(__file__)]},'scope':r['scope']};out=D/'dedup-local-rank-cached-process-timing-audit.json';assert not out.exists();out.write_text(json.dumps(result,indent=2)+'\n');print(result['summary'],result['observed_median_ratio'])
