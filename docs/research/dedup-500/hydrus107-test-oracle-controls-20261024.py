import ast,pathlib,tempfile,types,unittest,json,hashlib
b=pathlib.Path.cwd()/'docs/research/dedup-500';ev=b/'evidence/hydrusvideodeduplicator--hydrus-video-deduplicator';p=ev/'13-test_vpdqpy.py';tree=ast.parse(p.read_text());cls=next(n for n in tree.body if isinstance(n,ast.ClassDef) and n.name=='TestVpdq')
class Hash:
 def __len__(self):return 1
 def __str__(self):return 'owned-generated-current-hash'
 def __eq__(self,other):return isinstance(other,Hash)
class Algorithm:
 @staticmethod
 def computeHash(path):return Hash()
 @staticmethod
 def is_similar(a,b):return True,100.
g={'__name__':'owned_vpdq_oracle','unittest':unittest,'logging':__import__('logging'),'Path':pathlib.Path,'Vpdq':Algorithm,'VpdqHash':Hash,'vpdq':types.SimpleNamespace(VpdqHash=types.SimpleNamespace(from_string=lambda s:Hash()))}
exec(compile(ast.fix_missing_locations(ast.Module(body=[ast.ImportFrom(module='__future__',names=[ast.alias(name='annotations')],level=0),cls],type_ignores=[])),str(p),'exec'),g);C=g['TestVpdq'];results=[]
with tempfile.TemporaryDirectory(prefix='owned-vpdq-oracle-') as td:
 obj=C();obj.similarity_vids=[pathlib.Path(td)/'S01_owned.mp4'];obj.strange_vids=[];obj.video_hashes_dir=pathlib.Path(td);target=pathlib.Path(td)/'S01_owned.mp4.txt';assert not target.exists();obj.test_hashing_identical();assert target.read_text()=='owned-generated-current-hash';results.append({'control':'missing_baseline_created_by_current_algorithm_and_accepted','pass':True})
 obj.similarity_vids=[];obj.strange_vids=[]
 for method in ['test_hashing','test_hashing_identical','test_compare_similarity_true']:getattr(obj,method)()
 results.append({'control':'empty_video_inventory_all_three_methods_pass','methods':3,'pass':True})
report={'repository':'hydrusvideodeduplicator/hydrus-video-deduplicator','scope':'Unchanged upstream TestVpdq class AST-extracted; setup omitted, owned temp paths and replacement hash backend. Authored oracle controls, not original native suite results.','source_sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'controls':results};(b/'hydrus107-test-oracle-controls-20261024.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
