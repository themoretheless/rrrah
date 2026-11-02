"""Complete original SelectionStage and binary loader; owned finite bucket files.

Adapters: Stage timing/stats, local folders/output manager, log/progress only.
No MinHash generation, distributed job, optional signature analysis or corpus.
"""
import ast
import contextlib
from collections import defaultdict,Counter
from dataclasses import dataclass,asdict
import __future__
import hashlib
import io
import itertools
import json
from pathlib import Path
import struct

source=Path(__file__).resolve().parents[1]/'evidence/icodywu--minhash-dedup/02-minhash.py'
tree=ast.parse(source.read_text());wanted={'_pack_dup_key','_unpack_dup_key','_load_dup_buckets','SelectionStage'}
nodes=[n for n in tree.body if isinstance(n,(ast.FunctionDef,ast.ClassDef)) and n.name in wanted]
class Stage:
    def track_time(self):return contextlib.nullcontext()
    def stat_update(self,*a):pass
class Log:
    def info(self,*a,**k):pass
class Manager:
    def __init__(self,root):self.root=root;self.files={}
    def __enter__(self):return self
    def write(self,path,data):
        if path not in self.files:self.files[path]=(self.root/path).open('wb')
        self.files[path].write(data)
    def __exit__(self,*args):
        for f in self.files.values():f.close()
class Folder:
    def __init__(self,path):self.root=path;path.mkdir(parents=True,exist_ok=True)
    def list_files(self,glob_pattern):return sorted(p.name for p in self.root.glob(glob_pattern))
    def open(self,path,mode):return (self.root/path).open(mode)
    def get_output_file_manager(self,mode):assert mode=='wb';return Manager(self.root)
@dataclass
class Config:
    fixture_only:bool=True
scope=dict(Stage=Stage,struct=struct,io=io,defaultdict=defaultdict,asdict=asdict,SENTINEL=(1<<32)-1,get_datafolder=lambda x:x,logger=Log(),tqdm=lambda iterable,**kwargs:iterable)
exec(compile(ast.Module(body=nodes,type_ignores=[]),str(source),'exec',flags=__future__.annotations.compiler_flag),scope)
root=Path(__file__).with_name('owned-selection-fixtures');root.mkdir(exist_ok=True)
def run(name,rows):
    inp=Folder(root/name/'input');out=Folder(root/name/'output')
    with inp.open('000_00.dups','wb') as f:
        for row in rows:
            f.write(struct.pack('<I',len(row)))
            for key in row:f.write(struct.pack('<II',0,key))
    stage=scope['SelectionStage'](inp,inp,out,config=Config(),analyze_clustering=False)
    stage.run()
    metadata={}
    for p in out.root.glob('*.clusters'):
        for doc,cluster,size in struct.iter_unpack('<III',p.read_bytes()):metadata[doc]=(cluster,size)
    removed=set()
    for p in out.root.glob('*.remove'):
        removed.update(x[0] for x in struct.iter_unpack('<I',p.read_bytes()))
    expected=set().union(*(set(row) for row in rows)) if rows else set()
    assert set(metadata)==expected,(name,rows,metadata)
    clusters=defaultdict(set)
    for doc,(cluster,size) in metadata.items():clusters[cluster].add(doc)
    for members in clusters.values():
        reps=members-removed;assert len(reps)==1,(name,members,removed)
        rep=next(iter(reps))
        assert all(metadata[doc][1]==len(members) for doc in members)
        assert all(doc==rep or any(doc in row and rep in row for row in rows) for doc in members),(name,rows,members,rep)
    return {'removed':sorted(removed),'clusters':[sorted(v) for v in clusters.values()]}
chain=run('chain',[[1,2],[2,3]])
assert len(chain['clusters'])==2 and chain['removed']==[2]
edges=[list(c) for n in range(1,4) for c in itertools.combinations(range(3),n)]
for mask in range(1<<len(edges)):run(f'finite-{mask:03d}',[e for i,e in enumerate(edges) if mask&(1<<i)])
result={'scope':__doc__,'source_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),'authored_controls_run':2,'finite_graphs_checked':128,'chain':chain,'verified_invariants':['all input document IDs assigned','exactly one nonremoved representative per cluster','written cluster size matches members','each nonrepresentative shares an original bucket with its representative'],'original_tests_run':0}
Path(__file__).with_name('selection-results.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2))
