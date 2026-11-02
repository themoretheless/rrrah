"""Complete original PruneStage with local-folder, timing/log/progress adapters.

Actual owned binary files and original run/process/rewrite algorithms; no
signature generation, distributed execution or full corpus pipeline.
"""
import ast
import contextlib
from collections import defaultdict
import hashlib
import io
import json
import os
from pathlib import Path
import re
import struct
import __future__

source=Path(__file__).resolve().parents[1]/'evidence/icodywu--minhash-dedup/02-minhash.py'
tree=ast.parse(source.read_text());cls=next(n for n in tree.body if isinstance(n,ast.ClassDef) and n.name=='PruneStage')
class Stage:
    def track_time(self):return contextlib.nullcontext()
class Logger:
    def info(self,*a,**k):pass
    def warning(self,*a,**k):pass
class Folder:
    def __init__(self,path):self.path=str(path);path.mkdir(parents=True,exist_ok=True)
    def open(self,path,mode):return (Path(self.path)/path).open(mode)
    def list_files(self,glob_pattern):return sorted(p.name for p in Path(self.path).glob(glob_pattern))
scope=dict(Stage=Stage,struct=struct,re=re,io=io,os=os,defaultdict=defaultdict,get_datafolder=lambda folder:folder,logger=Logger(),tqdm=lambda iterable,**kwargs:iterable)
exec(compile(ast.Module(body=[cls],type_ignores=[]),str(source),'exec',flags=__future__.annotations.compiler_flag),scope)
root=Path(__file__).with_name('owned-prune-fixtures');root.mkdir(exist_ok=True)
def write(folder,name,rows):
    with folder.open(name,'wb') as f:
        for row in rows:
            f.write(struct.pack('<I',len(row)))
            for doc in row:f.write(struct.pack('<II',0,doc))
def run_case(name,bands,alias=False):
    inp=Folder(root/name/'input');out=inp if alias else Folder(root/name/'output')
    for band,rows in enumerate(bands):write(inp,f'{band:03d}_00.dups',rows)
    stage=scope['PruneStage'](inp,out)
    for rank in range(len(bands)):stage.run(rank=rank,world_size=len(bands))
    original_input=stage.input_folder;stage.input_folder=out
    rows={f:list(stage._iter_bucket_rows(f)) for f in out.list_files('*.dups')}
    stage.input_folder=original_input
    return {name:[[scope['PruneStage']._unpack_key(k) for k in row] for _,row in values] for name,values in rows.items()}
subset=run_case('subset',[[[1,2]],[[1,2,3]]]);assert subset['000_00.dups']==[] and subset['001_00.dups']==[[(0,1),(0,2),(0,3)]]
equal=run_case('equal',[[[1,2]],[[1,2]]]);assert equal['000_00.dups']==[[(0,1),(0,2)]] and equal['001_00.dups']==[]
alias=run_case('alias',[[[1,2]]],True);assert alias['000_00.dups']==[]
result={'scope':__doc__,'source_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),'authored_controls_run':3,'original_tests_run':0,'observations':[{'case':'strict subset dropped and superset preserves all members','output':subset},{'case':'equal cross-band buckets keep lower UID only','output':equal},{'case':'same input/output directory truncates owned source file before reading it','output':alias,'qualification':'boundary invocation only; normal scripts/config folder separation not yet qualified'}]}
Path(__file__).with_name('prune-results.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2))
