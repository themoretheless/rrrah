import sys,types,pathlib,importlib.util,tempfile,json,hashlib
base=pathlib.Path.cwd()/'docs/research/dedup-500';ev=base/'evidence/hydrusvideodeduplicator--hydrus-video-deduplicator'
for name in ['hydrusvideodeduplicator','hydrusvideodeduplicator.db','hydrusvideodeduplicator.__about__','rich','hvdaccelerators']:
 m=types.ModuleType(name);m.__path__=[];sys.modules[name]=m
sys.modules['rich'].print=lambda *a,**kw:None
sys.modules['hydrusvideodeduplicator.__about__'].__version__='0.10.0'
def match(a,b,t):return 100-sum((x^y).bit_count() for x,y in zip(a,b))
sys.modules['hvdaccelerators'].vpdq=types.SimpleNamespace(matchHashBytes=match)
def load(name,p):
 s=importlib.util.spec_from_file_location(name,p);m=importlib.util.module_from_spec(s);sys.modules[name]=m;s.loader.exec_module(m);return m
t=load('hydrusvideodeduplicator.db.vptree',ev/'12-vptree.py');d=load('hydrusvideodeduplicator.db.DedupeDB',ev/'10-DedupeDB.py');results=[]
with tempfile.TemporaryDirectory(prefix='owned-tree107-') as td:
 db=d.DedupeDb(pathlib.Path(td),'owned.sqlite');db.init_connection();db.create_tables();tree=t.VpTreeManager(db)
 try:tree.regenerate_tree()
 except IndexError:results.append({'control':'empty_tree_regeneration','observed':'IndexError','pass':True})
 else:raise AssertionError('expected empty root selection failure')
 for name,h in [('a',b'\x00'),('b',b'\x00'),('c',b'\xff'),('d',b'\x0f')]:
  db.add_file(name);db.add_perceptual_hash(h);db.associate_file_with_perceptual_hash(name,h)
 exact=tree.search_file(db.get_hash_id('a'),0);assert {i for i,dist in exact}=={db.get_hash_id('a'),db.get_hash_id('b')};results.append({'control':'shared_hash_maps_both_files','pass':True})
 checks=0
 for name in ['a','b','c','d']:
  fid=db.get_hash_id(name);h=db.get_phash(db.get_phash_id_from_hash_id(fid))
  for threshold in [1,4,5,8,9]:
   expected={db.get_hash_id(other) for other in ['a','b','c','d'] if t.calculate_distance(h,db.get_phash(db.get_phash_id_from_hash_id(db.get_hash_id(other))))<=threshold};actual={i for i,dist in tree.search_file(fid,threshold)};assert expected==actual,(name,threshold,expected,actual);checks+=1
 results.append({'control':'tree_against_exhaustive_metric_oracle','queries':checks,'pass':True});db.close()
report={'repository':'hydrusvideodeduplicator/hydrus-video-deduplicator','scope':'Unchanged complete Python tree and DB; real SQLite. Native matchHashBytes replaced by deterministic Hamming-derived similarity. Does not establish native VPDQ metric properties or recall.','controls':results,'tree_sha256':hashlib.sha256((ev/'12-vptree.py').read_bytes()).hexdigest()};(base/'hydrus107-tree-controls-20261024.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
