from pathlib import Path
import os,subprocess,time,json,statistics,hashlib
root=Path.cwd();report=root/'docs/research/ai-precolor-buffer-paired-2026-10-13.json'
env=os.environ.copy();env.update(RRRAH_AI_REPLAY_CORPUS=str(root/'tests/fixtures/data/metal-recovery/ai-corpus'),RRRAH_AI_SPATIAL_PIXELS='147,59;145,50;199,25;138,39;149,56',RRRAH_AI_DIAGNOSTIC_PIXEL_WORK='16000000')
for k in ['RRRAH_TRACE_SPATIAL_ROUTE','RRRAH_AI_FRAGMENT_MOMENTS','RRRAH_AI_SOURCE_FIELD_MOMENTS','RRRAH_AI_GEOMETRY_FIELD_KEYS','RRRAH_AI_RASTER_TRANSITIONS','RRRAH_AI_OWNER_TRANSITIONS','RRRAH_AI_PRECOLOR_KEYS']:env[k]='1'
expected=[s for s in (root/'docs/research/ai-precolor-buffer-original-2026-10-13.log').read_text().splitlines() if s.startswith('REAL_AI_SPATIAL_PIXEL')]
rows=[];order='ABBAABBA';bins={'A':'/tmp/rrrah-precolor-buffer-before-20261013','B':'/tmp/rrrah-precolor-buffer-after-20261013'}
for i,v in enumerate(order):
 start=time.perf_counter();proc=subprocess.run([bins[v],'real_ai_spatial_scene_problem_pixel_diagnostic','--ignored','--nocapture'],env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,timeout=60);elapsed=time.perf_counter()-start
 log=root/f'docs/research/ai-precolor-buffer-paired-{i+1}-{v}-2026-10-13.log';log.write_text(proc.stdout)
 assert proc.returncode==0 and 'test result: ok. 1 passed' in proc.stdout
 assert [s for s in proc.stdout.splitlines() if s.startswith('REAL_AI_SPATIAL_PIXEL')]==expected
 assert all('mismatches=0' in s for s in proc.stdout.splitlines() if s.startswith('SPATIAL_PRECOLOR_FIELD_KEYS'))
 rows.append({'variant':v,'seconds':elapsed,'log':log.name});print(v,round(elapsed,4),flush=True)
 report.write_text(json.dumps({'scope':'Five original AI diagnostic pixels, all observer flags active; not normal viewer throughput','order':order,'completed_runs':len(rows),'runs':rows,'binary_sha256':{v:hashlib.sha256(Path(p).read_bytes()).hexdigest() for v,p in bins.items()},'median_seconds':{v:statistics.median(x['seconds'] for x in rows if x['variant']==v) for v in bins if any(x['variant']==v for x in rows)},'all_original_pixels_exact':True},indent=2)+'\n')
