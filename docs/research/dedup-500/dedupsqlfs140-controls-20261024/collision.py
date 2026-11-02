import ast,io,json,types,sqlite3
from time import time
from pathlib import Path
b=Path('docs/research/dedup-500/evidence/sergey-dryabzhinsky--dedupsqlfs');tree=ast.parse((b/'deep-operations.py').read_text());owner=next(c for c in tree.body if isinstance(c,ast.ClassDef) and any(isinstance(m,ast.FunctionDef) and m.name=='__write_block_data' for m in c.body));method=next(m for m in owner.body if isinstance(m,ast.FunctionDef) and m.name=='__write_block_data');minimal=ast.ClassDef(name=owner.name,bases=[],keywords=[],body=[method],decorator_list=[]);ns={'time':time,'constants':types.SimpleNamespace(COMPRESSION_TYPE_NONE='none')};exec(compile(ast.fix_missing_locations(ast.Module(body=[minimal],type_ignores=[])),'original-write-block','exec'),ns);C=ns[owner.name];mangled='_'+owner.name+'__'
class SqliteHashes:
 def __init__(self):self.db=sqlite3.connect(':memory:');self.db.execute('create table hashes(id integer primary key,value blob unique)');self.db.execute('insert into hashes values(7,?)',(b'constant',))
 def find(self,v):r=self.db.execute('select id from hashes where value=?',(v,)).fetchone();return r[0] if r else None
 def insert(self,v):return self.db.execute('insert into hashes(value) values(?)',(v,)).lastrowid
class Quiet:
 def __init__(self):self.errors=[];self.inserts=[]
 def debug(self,*x):pass
 def error(self,*x):self.errors.append(x)
 def warn(self,*x):self.errors.append(x)
 def insert(self,*x):self.inserts.append(x)
 def find(self,*x):return True
 def inc(self,*x):pass
 def hashHasOwner(self,*x):return True
 def set(self,*x):pass

def fixture(old,hash_fn=lambda _:b'constant'):
 obj=C();q=Quiet();tables={'hash':SqliteHashes(),'inode_hash_block':q,'hash_count':q,'hash_owner':q,'block':types.SimpleNamespace(get=lambda _:dict(data=old))};obj.getTable=lambda name:tables[name];obj.getLogger=lambda:q;obj.getMyUuid=lambda:'owned';obj.getOption=lambda name:name=='collision_check_enabled';obj.getCompressionTypeName=lambda _:'none';obj.do_hash=hash_fn;obj.cached_indexes=q;obj.reportHelper=types.SimpleNamespace(bytes_written=0,bytes_deduped=0,bytes_db_written=0,time_spent_writing_blocks=0);setattr(obj,mangled+'get_index_from_cache',lambda *_:None);setattr(obj,mangled+'get_compression_type_by_hash_from_cache',lambda _:1);setattr(obj,mangled+'decompress',lambda data,_:data);return obj,q
obj,log=fixture(b'AAAA');result=getattr(obj,mangled+'write_block_data')(1,0,io.BytesIO(b'BBBB'));assert result['hash']==7 and result['data'] is None and result['new'] is False and obj.reportHelper.bytes_deduped==4 and log.errors
same,_=fixture(b'BBBB');ok=getattr(same,mangled+'write_block_data')(1,0,io.BytesIO(b'BBBB'));assert ok['hash']==7
bad,_=fixture(b'AAAA',lambda data:b'other' if data==b'AAAA' else b'constant')
try:getattr(bad,mangled+'write_block_data')(1,0,io.BytesIO(b'BBBB'));raise AssertionError('corruption accepted')
except RuntimeError as e:assert str(e)=='Data corruption!'
out={'controls_passed':3,'scope':'Original private write-block method AST unchanged;injected hash collision;real SQLite lookup fixture;explicit cache/owner/index/compression/logger adapters;no real hash collision,FUSE or actual full database manager execution','findings':['With collision checking enabled and differing bytes but equal injected digest,method logs warning and still reuses old hash id with no replacement data.','Equal content positive reuses id.','Different recomputed old digest raises Data corruption.'],'warning_count':len(log.errors)};Path(__file__).with_name('collision-results.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out))
