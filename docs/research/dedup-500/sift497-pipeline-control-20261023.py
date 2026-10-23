import json,queue,sys,tempfile,types
from pathlib import Path
root=Path('/Users/themoretheless/Documents/Sources/rrrah/.dedup-work/sift497-20261023/viz/sift_viz')
pkg=types.ModuleType('sift_viz');pkg.__path__=[str(root)];sys.modules['sift_viz']=pkg
media=types.ModuleType('sift_viz.media');media.log_stderr=lambda *args:None;sys.modules['sift_viz.media']=media
from sift_viz.models import HashSettings
from sift_viz.pipeline import PipelineRunner,_parse_progress
with tempfile.TemporaryDirectory(prefix='sift-pipeline-review-') as td:
 work=Path(td);binary=work/'successful-no-output';binary.write_text('#!/bin/sh\nexit 0\n');binary.chmod(0o700)
 for name in ['projection_2d.json','projection_3d.json','clusters.json']:(work/name).write_text(json.dumps({'marker':'stale'}))
 q=queue.Queue();r=PipelineRunner(str(binary),work,q);r._pipeline_thread(HashSettings(),'unused-controlled-folder')
 events=[]
 while not q.empty():events.append(q.get())
 done=next(e for e in events if e['kind']=='pipeline_done');assert done['projection_2d']=={'marker':'stale'} and not r.hashes_path.exists()
 r.cancel();r._projection_thread(HashSettings());events=[]
 while not q.empty():events.append(q.get())
 assert any(e['kind']=='projection_done' for e in events)
 r._cluster_thread(HashSettings());events=[]
 while not q.empty():events.append(q.get())
 assert any(e['kind']=='cluster_done' and e['clusters']=={'marker':'stale'} for e in events)
 assert _parse_progress('sift: progress 9/0')==(9,0)
 print('actual subprocesses: zero-exit/no-output reuses stale projections; projection+cluster run despite prior cancel; invalid progress denominator accepted')
