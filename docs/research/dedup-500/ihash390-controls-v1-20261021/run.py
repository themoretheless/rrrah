"""Original class AST with synthetic integer hashes/dicts; no images or Redis."""
from pathlib import Path
import ast,types
R=Path(__file__).resolve().parents[4]
s=(R/'docs/research/dedup-500/evidence/matteounitn--iHashDNA/02-ihashdna.py').read_text()
class Hash:
 def __init__(self,n):self.n=n;self.hash=[None]*8
 def __str__(self):return str(self.n)
 def __sub__(self,other):return (self.n^other.n).bit_count()
class Lock:
 def __init__(self,**kwargs):pass
 def __enter__(self):return self
 def __exit__(self,*args):pass
ns={'im':types.SimpleNamespace(ImageHash=Hash,hex_to_hash=lambda s:Hash(int(s))),'Redlock':Lock}
t=ast.parse(s);t.body=[x for x in t.body if isinstance(x,ast.ClassDef)];exec(compile(t,'original-ihashdna-class','exec'),ns)
def make():
 o=ns['ihashdna'].__new__(ns['ihashdna']);o.redis=None
 o.phashdb={'1':0};o.whashdb={};o.phashdb_clean={};o.whashdb_clean={}
 o.get_rotations=lambda raw:[raw]*4
 return o
o=make();assert o.check_hash(Hash(0),True,threshold=0)==(False,1.56)
assert o.check_hash(Hash(0),True,threshold=2)==(False,-2)
print('PASS cached no-match survives a later relaxed threshold (original check_hash, synthetic hashes)')
o=make();assert o.check_hash(Hash(0),True,threshold=2)==(True,1.56)
print('PASS fresh positive control recognizes the same relaxed-threshold match')
o=make();a=o.calc_similar_hash({'255':0,'1':0},Hash(0),'0',0,20)
b=o.calc_similar_hash({'1':0,'255':0},Hash(0),'0',0,20)
assert a==(True,12.5) and b==(True,1.56)
print('PASS returned accepted distance depends on dictionary order; not a nearest-neighbour contract')
o=make();db={'0':0}
try:o.exec_similar_hash(db,Hash(0),0,13,o.del_from_db)
except KeyError:assert db=={};print('PASS symmetric rotation adapters cause repeated deletion KeyError after first delete')
else:raise AssertionError('Expected repeated-key deletion failure')
print('4 authored controls passed; no original pHash/wHash, images, numpy, Redis, or locks executed')
