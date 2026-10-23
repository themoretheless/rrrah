import ast, contextlib, json, logging, os, sys, tempfile, time, types
from pathlib import Path
SOURCE = Path(__file__).resolve().parent / "intra095-20261024"
def original_method(path, cls, name, env):
    tree=ast.parse((SOURCE/path).read_text())
    node=next(n for c in tree.body if isinstance(c,ast.ClassDef) and c.name==cls for n in c.body if isinstance(n,ast.FunctionDef) and n.name==name)
    exec(compile(ast.Module(body=[node],type_ignores=[]),str(SOURCE/path),"exec"),env)
    return env[name]
class Cursor:
    def execute(self,*args): self.query=args
    def fetchall(self): return [(1,0),(2,None),(3,7),(4,-1)]
cur=Cursor()
db=types.SimpleNamespace(tableName="owned",transaction=lambda:contextlib.nullcontext(cur))
rows=original_method("dbPhashApi.py","PhashDbApi","getRandomPhashRows",{})(db)
assert rows=={(3,7),(4,-1)}
class Queue:
    def __init__(self): self.items=[]
    def put(self,x): self.items.append(x)
    def qsize(self): return len(self.items)
queue=Queue()
with tempfile.TemporaryDirectory(prefix="rrrah-intra095-owned-") as root:
    base=Path(root)
    for folder in ["skip","skip-other","keep"]:
        (base/folder).mkdir(); (base/folder/"one.bin").write_bytes(b"owned")
    worker=types.SimpleNamespace(log=logging.getLogger("owned"),mask_path=[str(base/"skip")],toProcessQueue=queue,checkIntegrity=False)
    cfg=types.SimpleNamespace(sourcePath=str(base),purge=False,noCleanTemps=True)
    env=dict(os=os,sys=sys,time=time,tqdm=types.SimpleNamespace(tqdm=lambda x,**kw:x),scanner=types.SimpleNamespace(runState=types.SimpleNamespace(run=True)))
    original_method("scanner/scanner.py","DedupScanTool","queueFolderContents",env)(worker,cfg)
    queued=sorted(str(Path(x[0]).relative_to(base)) for x in queue.items)
    assert queued==["keep/one.bin"]
result=dict(zero_hash_row_retained=(1,0) in rows,nonzero_hash_rows_retained=len(rows),queued_relative_paths=queued,unrelated_skip_other_omitted="skip-other/one.bin" not in queued,scope="Exact AST-selected original method bodies; owned paths and cursor/queue/progress adapters; no PostgreSQL, worker threads, hashing, archive extraction or deletions")
print(json.dumps(result,sort_keys=True))
