import ast,json,logging,os,tempfile,types
from pathlib import Path
p=Path(__file__).resolve().parent/'intra095-20261024/deduplicator/ProcessArchive.py'
tree=ast.parse(p.read_text());cls=next(x for x in tree.body if isinstance(x,ast.ClassDef) and x.name=='ArchChecker')
methods=[x for x in cls.body if isinstance(x,ast.FunctionDef) and x.name in {'_getBinaryMatchesForHash','_doRowLookup'}]
env=dict(os=os);exec(compile(ast.Module(body=methods,type_ignores=[]),str(p),'exec'),env)
with tempfile.TemporaryDirectory(prefix='rrrah-intra095-owned-') as d:
 path=Path(d)/'skip-other'/'excluded-keyword.zip';path.parent.mkdir();path.write_bytes(b'owned')
 row=(1,str(path),'member','digest',7,'image',10,10)
 db=types.SimpleNamespace(getByHash=lambda *a,**kw:[(str(path),'member')],getItem=lambda **kw:row,deleteDbRows=lambda **kw:(_ for _ in ()).throw(AssertionError('unexpected mutation')))
 c=types.SimpleNamespace(db=db,archPath='owned-source.zip',negativeMaskedPaths=[],positiveMaskedPaths=[],negativeKeywords=['excluded-keyword'],log=logging.getLogger('owned'))
 binary=env['_getBinaryMatchesForHash'](c,'digest');phash=env['_doRowLookup'](c,[1],(10,10))
 assert not binary and len(phash)==1
 c.negativeKeywords=[];c.negativeMaskedPaths=[str(Path(d)/'skip')]
 prefix_binary=env['_getBinaryMatchesForHash'](c,'digest');prefix_phash=env['_doRowLookup'](c,[1],(10,10))
 assert not prefix_binary and not prefix_phash
 print(json.dumps(dict(keyword_binary_count=len(binary),keyword_phash_count=len(phash),prefix_binary_count=len(prefix_binary),prefix_phash_count=len(prefix_phash),scope='Exact AST-selected original methods;real owned candidate path plus query/row adapters;no PostgreSQL or file mutations'),sort_keys=True))
