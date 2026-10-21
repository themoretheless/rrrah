from pathlib import Path
import sys,types,runpy,json,tempfile,hashlib,importlib.util
import numpy as np
C=Path('.dedup-work/phasher415-20261021');sys.modules['imagehash']=types.ModuleType('imagehash')
h=runpy.run_path(str(C/'hamming.py'));u=runpy.run_path(str(C/'union_find.py'));mat=h['build_hash_matrix'](['0','1','3']);pairs=h['vectorized_pairwise_below'](mat,1);uf=u['UnionFind'](3)
for a,b,d in pairs:uf.union(a,b)
assert uf.find(0)==uf.find(2) and int(h['vectorized_hamming_1_vs_n'](mat[0],mat[2:])[0])==2
ref=np.zeros((1,256),dtype=np.uint64);target=np.full((1,256),np.iinfo(np.uint64).max,dtype=np.uint64);idx,dist=h['vectorized_hamming_best'](ref,target,20000);assert int(idx[0])==-1 and int(dist[0])==9999
pkg=types.ModuleType('p415');pkg.__path__=[str(C.resolve())];sys.modules['p415']=pkg
ui=types.ModuleType('p415.ui')
for n in ['info','warn','error']:setattr(ui,n,lambda *a,**k:None)
ui.get_images=lambda folder:[folder/'image.png'];sys.modules['p415.ui']=ui
tq=types.ModuleType('tqdm');tq.tqdm=lambda iterable,**kwargs:iterable;sys.modules['tqdm']=tq
spec=importlib.util.spec_from_file_location('p415.hashing',C/'hashing.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);calls=[]
def compute(p,size):calls.append(str(p));return str(p),hashlib.sha256(p.read_bytes()).hexdigest()
m.compute_phash=compute
with tempfile.TemporaryDirectory() as td:
 folder=Path(td);p=folder/'image.png';p.write_bytes(b'first');first=m.hash_folder(folder);p.write_bytes(b'changed');second=m.hash_folder(folder);assert first==second and len(calls)==1
r={'original_hamming_union_cache_functions_executed':True,'controls':{'transitive_group_endpoints_distance':2,'edge_threshold':1,'all_three_in_one_group':True,'16384bit_distance_best_index':int(idx[0]),'16384bit_distance_reported':int(dist[0]),'expected_distance':16384,'cache_reused_after_content_change':True,'compute_calls':len(calls)},'adapters':'Dummy imagehash import for vector math; cache uses deterministic byte-digest compute adapter and owned temporary directory, original hash_folder unchanged.','limits':['No imagehash perceptual computation, command UI, file deletion or production file mutation.','Transitive cluster behavior reproduced, not every pair within threshold.']};Path('docs/research/dedup-500/phasher415-controls-20261021/result.json').write_text(json.dumps(r,indent=2)+'\n');print(json.dumps(r))
