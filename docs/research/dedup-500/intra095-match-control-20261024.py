import ast,json,logging,os,tempfile,types
from pathlib import Path
source=Path(__file__).resolve().parent/'intra095-20261024/deduplicator/ProcessArchive.py'
tree=ast.parse(source.read_text());original=next(x for x in tree.body if isinstance(x,ast.ClassDef) and x.name=='ArchChecker')
names={'getMatchingArchives','_getBestMatchingArchive','getPhashMatchingArchives','_checkHashesOk','_doHashSearches','_isBadPee','_shouldSkipFile'}
klass=ast.ClassDef(name='Checker',bases=[],keywords=[],body=[x for x in original.body if isinstance(x,ast.FunctionDef) and x.name in names],decorator_list=[])
class ArchiveProcessorException(Exception):pass
class InvalidArchivePhashContentsException(ArchiveProcessorException):pass
class InvalidArchiveMd5ContentsException(ArchiveProcessorException):pass
env=dict(os=os,BAD_PHASHES=[0,-9223372036854775808],PHASH_DISTANCE_THRESHOLD=2,InvalidArchivePhashContentsException=InvalidArchivePhashContentsException,InvalidArchiveMd5ContentsException=InvalidArchiveMd5ContentsException,dbApi=types.SimpleNamespace(hammingDistance=lambda a,b:((a^b)&((1<<64)-1)).bit_count()))
exec(compile(ast.fix_missing_locations(ast.Module(body=[klass],type_ignores=[])),str(source),'exec'),env)
Checker=env['Checker'];results=[]
for label,second_hash,second_count in [('unique_member',2,0),('dense_unresolved_member',2,101),('ignored_zero_member',0,1)]:
 c=Checker();c.log=logging.getLogger('owned')
 data=[('first',dict(hexHash='first',pHash=1,imX=10,imY=10,type='image/png')),('second',dict(hexHash='second',pHash=second_hash,imX=10,imY=10,type='image/png'))]
 c._loadFileContents=lambda:data
 c.db=types.SimpleNamespace(getByHash=lambda *a,**kw:[],searchPhashSet=lambda hashes,d:{1:[1],second_hash:list(range(100,100+second_count))})
 c._doRowLookup=lambda ids,res:[dict(fspath='owned-candidate.zip')] if ids else []
 matches=c.getPhashMatchingArchives(searchDistance=0)
 results.append(dict(case=label,matched_source_members={p:[list(k) for k in members] for p,members in matches.items()}))
assert results[0]['matched_source_members']=={}
assert len(results[1]['matched_source_members']['owned-candidate.zip'])==1
assert len(results[2]['matched_source_members']['owned-candidate.zip'])==1
with tempfile.TemporaryDirectory(prefix='rrrah-intra095-owned-') as d:
 p=Path(d);a=p/'a.zip';b=p/'b.zip';a.write_bytes(b'owned-first');b.write_bytes(b'owned-second')
 c=Checker();c.log=logging.getLogger('owned');c.archPath='owned-source.zip'
 c.arch=types.SimpleNamespace(iterHashes=lambda:iter([('first',dict(type='application/octet-stream',hexHash='first')),('second',dict(type='application/octet-stream',hexHash='second'))]))
 c._getBinaryMatchesForHash=lambda h:{str(a if h=='first' else b):{h}}
 matches=c.getMatchingArchives();best=c._getBestMatchingArchive(matches)
 split=dict(candidate_count=len(matches),max_source_members_per_candidate=max(map(len,matches.values())),source_members=2,best_match_returned=best is not None)
 assert split['candidate_count']==2 and split['best_match_returned']
print(json.dumps(dict(phash_cases=results,split_binary_coverage=split,scope='Exact AST-selected original matching methods;owned synthetic metadata,query/row adapters and temporary candidate files;no actual archive decoder,PostgreSQL,move or delete calls'),sort_keys=True))
