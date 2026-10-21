import ast,hashlib,logging,os,tempfile,time,types
from pathlib import Path
from collections import defaultdict
from typing import Dict,List,Optional,Tuple
base=Path('.dedup-work/dupeeraser315-20261021')
env=dict(hashlib=hashlib,logging=logging,os=types.SimpleNamespace(path=os.path,walk=os.walk,remove=lambda p:(_ for _ in ()).throw(AssertionError('deletion forbidden'))),time=time,defaultdict=defaultdict,Dict=Dict,List=List,Optional=Optional,Tuple=Tuple)
env['humanize']=types.SimpleNamespace(naturalsize=str)
env['tqdm']=types.SimpleNamespace(write=lambda _:None)
env['blake3']=types.SimpleNamespace()
env['xxhash']=types.SimpleNamespace()
env['mmh3']=types.SimpleNamespace()
def unexpected(*a,**k):raise ValueError('synthetic unavailable image')
env['Image']=types.SimpleNamespace(open=unexpected)
env['imagehash']=types.SimpleNamespace(average_hash=unexpected,dhash=unexpected,whash=unexpected,colorhash=unexpected)
env['tabulate']=unexpected
for file,name in [('comparison_method.py','ComparisonMethod'),('hash_performance.py','HashPerformance'),('duplicate_remover.py','DuplicateRemover')]:
 tree=ast.parse((base/'dupe_eraser/core'/file).read_text())
 node=next(n for n in tree.body if isinstance(n,ast.ClassDef) and n.name==name)
 exec(compile(ast.Module(body=[node],type_ignores=[]),'original-'+file,'exec'),env)
Remover=env['DuplicateRemover']
def run(method,algorithm,files):
 with tempfile.TemporaryDirectory(prefix='dupe315-') as folder:
  root=Path(folder)
  for name,b in files.items():(root/name).write_bytes(b)
  r=Remover(comparison_method=method,hash_algorithm=algorithm,show_progress=False,print_only=True)
  pairs=r.find_and_remove_duplicates(folder)
  assert {p.name:p.read_bytes() for p in root.iterdir()}==files
  return {Path(a).name:Path(b).name for a,b in pairs.items()}
files={'a.bin':b'AAAA','b.bin':b'BBBB','c.bin':b'BBBB'}
assert run('bytes','sha256',files)=={}
print('PASS counterexample: bytes mode misses b/c duplicate when same-size a differs')
assert run('hash','sha256',files)=={'c.bin':'b.bin'}
print('PASS real SHA256 mode finds b/c duplicate in same fixture')
assert run('hash','phash',{'a.txt':b'AAAA','b.txt':b'BBBB'})=={'b.txt':'a.txt'}
print('PASS perceptual non-image sentinel groups unequal same-size text files')
assert run('hash','phash',{'a.png':b'AAAA','b.png':b'BBBB'})=={'b.png':'a.png'}
print('PASS image-open failure sentinel groups unequal same-size invalid images (explicit unavailable-image adapter)')
print('TOTAL4passed; original classes withreal tempfile/read/SHA256,print-only; explicit optional-library adapters. No original deletion, actualPIL/ImageHash/fast-hash dependencies or wholeCLI')
