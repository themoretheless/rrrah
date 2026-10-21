import os,sys,json,subprocess,tempfile
from pathlib import Path
source=Path('.dedup-work/mediadedupe444-20261021').resolve();deps=Path('.dedup-work/mediahash478-extra-deps-20261021').resolve()
sys.path[:0]=[str(source),str(deps)]
import engine
out={}
with tempfile.TemporaryDirectory(prefix='rrrah-md444-') as tmp:
 root=Path(tmp);env=dict(os.environ,PYTHONPATH=str(deps),DUPFINDER_DATA=str(root/'data'));empty=root/'empty';empty.mkdir()
 p=subprocess.run([sys.executable,str(source/'cli.py'),'-d',str(empty)],env=env,text=True,capture_output=True,timeout=60)
 assert p.returncode==1 and "unexpected keyword argument 'video_error'" in p.stderr
 out['original_cli_empty_scan']=dict(returncode=p.returncode,stdout=p.stdout.replace(tmp,'<temp>'),stderr=p.stderr.replace(tmp,'<temp>'),report_exists=(root/'data/duplicates_report.html').exists())
 demo=root/'demo';p=subprocess.run([sys.executable,str(source/'create_demo_library.py'),str(demo)],text=True,capture_output=True,timeout=60);assert p.returncode==0
 paths=engine.scan_directory(demo);assert len(paths)==20
 db=engine.db_connect(root/'demo.sqlite');rows=engine.process_files(paths,db,workers=1);assert all(not x.get('error') for x in rows)
 exact=engine.find_exact_duplicates(rows);assert len(exact)==4
 exact_paths={r['path'] for g in exact for r in g};visual=engine.find_visual_duplicates(rows,exact_paths)
 rel=lambda p:str(Path(p).relative_to(demo))
 out['original_demo_engine']=dict(images=len(rows),exact_groups=[[rel(r['path']) for r in g] for g in exact],visual_groups=[[rel(r['path']) for r in g] for g in visual],scope='Original generator and actual Pillow/ImageHash engine; generated demonstration corpus, not independent held-out accuracy benchmark. CLI bug not patched or bypassed to claim CLI success.')
 db.close()
Path(__file__).with_name('result.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
