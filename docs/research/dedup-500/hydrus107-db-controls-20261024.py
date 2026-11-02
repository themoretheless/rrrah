import pathlib,types,sys,importlib.util,tempfile,json,unittest,io,hashlib
root=pathlib.Path.cwd();base=root/'docs/research/dedup-500'; ev=base/'evidence/hydrusvideodeduplicator--hydrus-video-deduplicator'
for name in ['hydrusvideodeduplicator','hydrusvideodeduplicator.db','hydrusvideodeduplicator.__about__','hydrusvideodeduplicator.db.vptree','rich']:
 m=types.ModuleType(name);m.__path__=[];sys.modules[name]=m
sys.modules['rich'].print=lambda *a,**kw:None
sys.modules['hydrusvideodeduplicator.__about__'].__version__='0.10.0'
class Tree:
 def __init__(self,*a):pass
 def add_leaf(self,*a):pass
sys.modules['hydrusvideodeduplicator.db.vptree'].VpTreeManager=Tree
def load(name,path):
 spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);sys.modules[name]=m;spec.loader.exec_module(m);return m
m=load('hydrusvideodeduplicator.db.DedupeDB',ev/'10-DedupeDB.py');test=load('owned_original_test_db',ev/'11-test_db.py');out=io.StringIO();r=unittest.TextTestRunner(stream=out,verbosity=2).run(unittest.defaultTestLoader.loadTestsFromModule(test));assert r.wasSuccessful()
controls=[]
with tempfile.TemporaryDirectory(prefix='owned-hydrus107-') as td:
 db=m.DedupeDb(pathlib.Path(td),'owned.sqlite');db.init_connection();db.create_tables();db.set_version('0.9.0')
 common='ab'*32
 for i,old in enumerate([[common+',31,0'],[common+',31,0','cd'*32+',0,1']],1):
  db.add_file('file'+str(i));pid=db.add_perceptual_hash(json.dumps(old));db.execute('INSERT INTO shape_perceptual_hash_map VALUES (?,?)',(pid,i))
 db.commit();assert db.upgrade_db()
 orphan=db.execute('SELECT phash_id,hash_id FROM shape_perceptual_hash_map WHERE phash_id NOT IN (SELECT phash_id FROM shape_perceptual_hashes)').fetchall();assert orphan==[(1,1)]
 controls.append({'name':'migration_converging_hashes_orphan_mapping','orphan_rows':orphan,'remaining_hash_rows':db.execute('SELECT count(*) FROM shape_perceptual_hashes').fetchone()[0],'pass':True})
 db.add_to_phashed_files_queue('owned',b'first');db.add_to_phashed_files_queue('owned',b'second');rows=db.execute('SELECT phash FROM phashed_file_queue WHERE file_hash=?',('owned',)).fetchall();assert rows==[(b'second',)];controls.append({'name':'queue_replacement_retains_latest_hash','pass':True});db.close()
report={'repository':'hydrusvideodeduplicator/hydrus-video-deduplicator','scope':'Unchanged DB and original test source loaded with replacement rich logger, package version 0.10.0, and no-op VpTreeManager. Real SQLite/temp files; no live Hydrus/native tree.','original_tests':{'run':r.testsRun,'failures':len(r.failures),'errors':len(r.errors),'output':out.getvalue()},'authored_controls':controls,'source_sha256':hashlib.sha256((ev/'10-DedupeDB.py').read_bytes()).hexdigest()};(base/'hydrus107-db-controls-20261024.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
