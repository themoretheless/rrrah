import ast,json,types,pathlib,hashlib
root=pathlib.Path.cwd(); base=root/'docs/research/dedup-500'; ev=base/'evidence/hydrusvideodeduplicator--hydrus-video-deduplicator'
p=ev/'03-dedup.py'; tree=ast.parse(p.read_text()); cls=next(n for n in tree.body if isinstance(n,ast.ClassDef) and n.name=='HydrusVideoDeduplicator'); method=next(n for n in cls.body if isinstance(n,ast.FunctionDef) and n.name=='find_potential_duplicates')
class Bar:
 def __init__(self,**kw): self.n=0; self.total=kw['total']
 def __enter__(self): return self
 def __exit__(self,*a): pass
 def update(self,n): self.n+=n
class DB:
 def execute(self,sql,*args):
  return types.SimpleNamespace(fetchall=lambda:[(1,),(2,)])
 def get_file_hash(self,n): return str(n)
 def commit(self): pass
class Tree:
 def __init__(self,db): pass
 def search_file(self,n,**kw): return [(3-n,0)]
g={'vptree':types.SimpleNamespace(VpTreeManager=Tree,fix_vpdq_similarity=lambda x:x),'tqdm':Bar};exec(compile(ast.fix_missing_locations(ast.Module(body=[method],type_ignores=[])),str(p),'exec'),g)
results=[]
for cancel in (False,True):
 marked=[]; calls=[]
 def stop(): calls.append(1);return cancel and len(calls)>1
 obj=types.SimpleNamespace(db=DB(),similarity_threshold=50,update_progress_callback=None,should_skip_step_fn=stop,hydlog=types.SimpleNamespace(info=lambda *a:None),mark_videos_as_duplicates=lambda a,b:marked.append((a,b)))
 count=g['find_potential_duplicates'](obj)
 expected=0 if cancel else 1
 assert count==expected and len(marked)==(1 if cancel else 2)
 results.append({'control':'cancel_after_first_file' if cancel else 'symmetric_full_search','reported_pairs':count,'relationship_calls':len(marked),'pass':True})
hcls=next(n for n in tree.body if isinstance(n,ast.ClassDef) and n.name=='FileHasher');hm=next(n for n in hcls.body if isinstance(n,ast.FunctionDef) and n.name=='_phash_file');hg={'compute_phash':lambda *a:types.SimpleNamespace(bytes=b''),'vpdq':types.SimpleNamespace(VpdqHash=types.SimpleNamespace(bytesPerPdqHash=32)),'FailedPerceptualHashException':RuntimeError};exec(compile(ast.fix_missing_locations(ast.Module(body=[hm],type_ignores=[])),str(p),'exec'),hg);assert hg['_phash_file'](types.SimpleNamespace(num_threads=1),b'owned')==b'';results.append({'control':'zero_byte_native_hash_accepted','pass':True})
report={'repository':'hydrusvideodeduplicator/hydrus-video-deduplicator','commit':'f3a13eee2dd9698c0805a859e46c95a2b6554efd','source_sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'scope':'Unchanged methods AST-extracted; recording DB/tree/progress/native hash replacements. No real Hydrus/PyAV/VPDQ or upstream test execution.','controls':results};(base/'hydrus107-native-controls-20261024.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
