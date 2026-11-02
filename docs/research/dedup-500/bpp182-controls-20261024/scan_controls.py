"""Original complete scan_images function; real owned files, logger-only adapter."""
import ast,json,logging,os,tempfile
from pathlib import Path
b=Path(__file__).resolve().parent;s=b.parent/'evidence/Arkalogy--best-photo-picker/03-io_scan.py';t=ast.parse(s.read_text());t.body=[x for x in t.body if not isinstance(x,ast.ImportFrom) or x.module!='bpp.utils.logging'];t.body=[x for x in t.body if not isinstance(x,ast.Assign) or not any(isinstance(a,ast.Name) and a.id=='log' for a in x.targets)];env={'log':logging.getLogger('scan-control')};exec(compile(t,str(s),'exec'),env);scan=env['scan_images'];out=[]
with tempfile.TemporaryDirectory(prefix='bpp182-scan-') as root:
 p=Path(root);(p/'data/nested').mkdir(parents=True);(p/'data/direct.jpg').write_bytes(b'a');(p/'data/nested/escaped.jpg').write_bytes(b'b');(p/'top.JPG').write_bytes(b'c');(p/'linked.jpg').symlink_to(p/'top.JPG')
 got=scan(root,extensions=['jpg'],recursive=True);rel=[str(Path(x).relative_to(p)) for x in got];assert rel==['data/nested/escaped.jpg','top.JPG'];out.append({'case':'skip_directory_does_not_prune_descendants','observed':rel})
 got=scan(root,extensions=['jpg']);assert [Path(x).name for x in got]==['top.JPG'];out.append({'case':'flat_scan_skips_symlink','observed':[Path(x).name for x in got]})
 got=scan(root,extensions=['jpg'],follow_symlinks=True);assert [Path(x).name for x in got]==['linked.jpg','top.JPG'];out.append({'case':'flat_scan_explicitly_includes_symlink','observed':[Path(x).name for x in got]})
 ticks=[];got=scan(root,extensions=['jpg'],recursive=True,max_images=1,on_progress=lambda a,z:ticks.append([a,z]));assert len(got)==1 and ticks[-1][1]==2;out.append({'case':'max_images_limits_result_not_traversal','returned':len(got),'final_progress':ticks[-1]})
 def fail(*args):raise RuntimeError('owned progress failure')
 assert scan(root,extensions=['jpg'],recursive=True,on_progress=fail);out.append({'case':'progress_callback_exception_contained','passed':True})
payload={'scope':__doc__,'controls_passed':len(out),'observations':out,'limits':'No real user paths, permission-denied IO, cyclic followed symlinks or performance qualification.'};(b/'scan-results.json').write_text(json.dumps(payload,indent=2)+'\n');print(json.dumps(payload,indent=2))
