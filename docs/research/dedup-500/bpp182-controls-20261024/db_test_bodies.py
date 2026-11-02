"""Eleven original DB test bodies; original full dedupe/constants modules.

Logger-module shim and explicit owned SQLite schema fixture replace application
init_db/migrations. No pytest fixture engine or full application schema tested.
"""
import ast,hashlib,importlib.util,json,logging,sqlite3,sys,traceback,types
from pathlib import Path
b=Path(__file__).resolve().parent;e=b.parent/'evidence/Arkalogy--best-photo-picker'
shim=types.ModuleType('bpp.utils.logging');shim.get_logger=logging.getLogger;sys.modules[shim.__name__]=shim
def load(name,path):
 spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);sys.modules[name]=m;spec.loader.exec_module(m);return m
const=load('bpp.constants',e/'remaining-pinned/bpp/constants.py');module=load('bpp.db.dedupe',e/'04-dedupe.py')
s=e/'remaining-pinned/tests/test_near_duplicate_clusters.py';t=ast.parse(s.read_text());nodes=[x for x in t.body if isinstance(x,ast.ClassDef) or isinstance(x,ast.FunctionDef) and x.name=='_insert'];env={};exec(compile(ast.fix_missing_locations(ast.Module(body=nodes,type_ignores=[])),str(s),'exec'),env)
def db():
 c=sqlite3.connect(':memory:');c.execute('CREATE TABLE photos (id INTEGER PRIMARY KEY, filepath TEXT, original_filename TEXT, file_size INTEGER, file_mtime REAL, phash INTEGER, ahash INTEGER, aggregate_score REAL, date TEXT, missing INTEGER DEFAULT 0, deleted_at TEXT, hidden_at TEXT, is_live_photo_sidecar INTEGER DEFAULT 0, dup_cluster_id INTEGER DEFAULT 0, cluster_size INTEGER DEFAULT 1)');return c
results=[]
for clsname in ['TestAssignNearDuplicateClusters','TestDuplicatesSmartAlbumUsesClusterSize']:
 obj=env[clsname]()
 for name in sorted(x for x in dir(obj) if x.startswith('test_')):
  c=db()
  try:getattr(obj,name)(c);results.append({'name':clsname+'.'+name,'passed':True})
  except Exception as ex:results.append({'name':clsname+'.'+name,'passed':False,'exception':repr(ex),'traceback':traceback.format_exc()})
  finally:c.close()
controls=[];c=db();insert=env['_insert'];insert(c,1,1234);insert(c,2,1234);assert module.assign_near_duplicate_clusters(c)==2;c.execute('UPDATE photos SET phash=NULL,ahash=NULL');c.commit();n=module.assign_near_duplicate_clusters(c);rows=c.execute('SELECT id,dup_cluster_id,cluster_size FROM photos ORDER BY id').fetchall();album=c.execute('SELECT count(*) FROM photos WHERE cluster_size>1 AND '+const.ACTIVE_PHOTO_SQL).fetchone()[0];assert n==0 and album==2;controls.append({'case':'real_cluster_then_all_hashes_invalidated_leaves_stale_album','new_clustered_count':n,'album_count':album,'rows':rows});c.close()
c=db();insert(c,1,1234,date='2024-07-01T10:00:00');insert(c,2,1234,date='2024-07-02T10:00:00');n=module.assign_near_duplicate_clusters(c);assert n==0;controls.append({'case':'identical_hashes_a_day_apart_remain_singletons','clustered':n,'qualification':'Conflicts with test-module documentation saying no time constraint;production doc/code explicitly60second window. Not proof currentpolicy unintended.'});c.close()
p={'scope':__doc__,'original_test_bodies_run':len(results),'passed':sum(x['passed'] for x in results),'failed':sum(not x['passed'] for x in results),'results':results,'authored_controls_passed':len(controls),'observations':controls,'source_sha256':{str(p.relative_to(e)):hashlib.sha256(p.read_bytes()).hexdigest() for p in [e/'04-dedupe.py',s,e/'remaining-pinned/bpp/constants.py']},'limits':'No original init_db/migrations,server/API/browser,real photo files,userdatabase or performance qualification.'};(b/'db-test-results.json').write_text(json.dumps(p,indent=2)+'\n');print(json.dumps(p,indent=2));assert all(x['passed'] for x in results)
