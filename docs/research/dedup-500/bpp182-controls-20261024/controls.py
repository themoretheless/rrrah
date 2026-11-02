"""Original AST functions, real owned in-memory SQLite; logger-only adapter."""
import ast,json,logging,sqlite3,hashlib
from pathlib import Path
base=Path(__file__).resolve().parent; evidence=base.parent/'evidence/Arkalogy--best-photo-picker'
def load(name, names, env):
 tree=ast.parse((evidence/name).read_text()); nodes=[x for x in tree.body if isinstance(x,(ast.FunctionDef,ast.Import,ast.ImportFrom)) and (not isinstance(x,ast.FunctionDef) or x.name in names) and not (isinstance(x,ast.ImportFrom) and x.module and x.module.startswith('bpp')) and not (isinstance(x,ast.Import) and any(a.name in ('cv2','numpy') for a in x.names))];nodes.insert(0,ast.ImportFrom(module='__future__',names=[ast.alias(name='annotations')],level=0));exec(compile(ast.fix_missing_locations(ast.Module(body=nodes,type_ignores=[])),name,'exec'),env)
env={'log':logging.getLogger('owned-bpp-controls'),'DEFAULT_HAMMING_THRESHOLD':8,'DEFAULT_TIME_WINDOW_SECONDS':60};load('04-dedupe.py',{'_hamming','_date_seconds_diff','_min_hash_distance','assign_near_duplicate_clusters'},env)
h={};load('05-phash.py',{'hamming_distance','dual_hash_distance'},h)
def db(rows):
 c=sqlite3.connect(':memory:');c.execute('CREATE TABLE photos(id INTEGER,phash INTEGER,ahash INTEGER,date TEXT,missing INTEGER DEFAULT 0,deleted_at TEXT,hidden_at TEXT,is_live_photo_sidecar INTEGER DEFAULT 0,dup_cluster_id INTEGER DEFAULT 9,cluster_size INTEGER DEFAULT 9)');c.executemany('INSERT INTO photos(id,phash,ahash,date) VALUES(?,?,?,?)',rows);return c
out=[]
for key,fn in [('db_hamming',env['_hamming']),('phash_hamming',h['hamming_distance'])]:
 actual=fn(-1,0); expected=((-1 ^ 0)&((1<<64)-1)).bit_count();assert (actual,expected)==(1,64);out.append({'case':key,'observed':actual,'fixed_width_64_reference':expected,'mismatch_reproduced':True})
c=db([(1,7,7,'2024-01-01T00:00:00'),(2,7,7,'2024-01-01T00:00:50'),(3,7,7,'2024-01-01T00:01:40')]);n=env['assign_near_duplicate_clusters'](c);rows=c.execute('SELECT id,dup_cluster_id,cluster_size FROM photos').fetchall();assert n==3 and all(r[2]==3 for r in rows);out.append({'case':'60_second_window_chain','endpoint_seconds':100,'returned':n,'rows':rows});c.close()
c=db([(1,None,None,'2024-01-01T00:00:00')]);n=env['assign_near_duplicate_clusters'](c);rows=c.execute('SELECT dup_cluster_id,cluster_size FROM photos').fetchall();assert n==0 and rows==[(9,9)];out.append({'case':'empty_candidate_early_return_leaves_previous_cluster','returned':n,'rows':rows});c.close()
c=db([(1,-1,None,'2024-01-01T00:00:00'),(2,0,None,'2024-01-01T00:00:00')]);n=env['assign_near_duplicate_clusters'](c);assert n==2;out.append({'case':'opposite_64bit_hashes_cluster_at_threshold8','returned':n,'true_distance':64});c.close()
p={'scope':__doc__,'controls_passed':len(out),'observations':out,'source_sha256':{f:hashlib.sha256((evidence/f).read_bytes()).hexdigest() for f in ['04-dedupe.py','05-phash.py']},'limits':'No cv2 image decoding, full original tests, real user database, performance or production adoption.'};(base/'results.json').write_text(json.dumps(p,indent=2)+'\n');print(json.dumps(p,indent=2))
