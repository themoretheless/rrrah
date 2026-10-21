import sys,os,tempfile,json,hashlib
from pathlib import Path
sys.path[:0]=[str(Path('.dedup-work/mediadedupe444-20261021').resolve()),str(Path('.dedup-work/mediahash478-extra-deps-20261021').resolve())]
import engine
from PIL import Image
out={}
with tempfile.TemporaryDirectory() as tmp:
 root=Path(tmp);a=root/'a.bmp';b=root/'b.bmp'
 Image.new('RGB',(16,16),'black').save(a);b.write_bytes(a.read_bytes());before=a.stat()
 db=engine.db_connect(root/'cache.sqlite')
 first=engine.process_files([a,b],db,workers=1)
 Image.new('RGB',(16,16),'white').save(a);os.utime(a,ns=(before.st_atime_ns,before.st_mtime_ns))
 assert a.stat().st_size==before.st_size
 second=engine.process_files([a,b],db,workers=1)
 assert hashlib.md5(a.read_bytes()).hexdigest()!=second[0]['md5']
 groups=engine.find_exact_duplicates(second);assert len(groups)==1
 out['restored_mtime_same_length_rewrite']={'false_exact_group_paths':[r['path'].split('/')[-1] for r in groups[0]],'cached_md5':second[0]['md5'],'actual_md5':hashlib.md5(a.read_bytes()).hexdigest(),'bytes_equal':a.read_bytes()==b.read_bytes()}
 db.close()
rows=[dict(path=str(i),size=s,md5='same') for i,s in enumerate([1,1,2,2])]
g=engine.find_exact_duplicates(rows);assert len(g)==1 and len(g[0])==4
out['synthetic_digest_collision_cross_size']={'sizes_in_one_exact_group':[r['size'] for r in g[0]],'scope':'Injected equal MD5 labels, not generated real MD5 collision; actual original grouping function'}
rows=[dict(path=n,phash=f'{v:016x}') for n,v in [('A',0),('B',1),('C',3)]]
g=engine.find_visual_duplicates(rows,set(),threshold=1);h=engine.find_visual_duplicates([rows[1],rows[0],rows[2]],set(),threshold=1)
assert [len(x) for x in g]==[2] and [len(x) for x in h]==[3]
out['visual_input_order']={'ABC':[[r['path'] for r in x] for x in g],'BAC':[[r['path'] for r in x] for x in h],'endpoint_A_C_distance':2,'threshold':1}
Path(__file__).with_name('result.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
