import ast,runpy,tempfile,json
from pathlib import Path
from simhash import Simhash,SimhashIndex
b=Path('docs/research/dedup-500/evidence/nmante--image_deduplication');tree=ast.parse((b/'03-near_duplicate.py').read_text());original=next(n for n in tree.body if isinstance(n,ast.ClassDef));names={'__init__','deduplicate_images','simhash_value_to_key','merge_near_duplicate_dictionaries'};c=ast.ClassDef(name='NearDuplicate',bases=[],keywords=[],body=[m for m in original.body if isinstance(m,ast.FunctionDef) and m.name in names],decorator_list=[]);ns={'Simhash':Simhash,'SimhashIndex':SimhashIndex};exec(compile(ast.fix_missing_locations(ast.Module(body=[c],type_ignores=[])),'original-grouping-methods','exec'),ns);N=ns['NearDuplicate']
def build(order,values):
 x=N(order,k=1);x.generate_features=lambda filename:values[filename];x.deduplicate_images();return x
values={'a':0,'b':1,'c':3};left=build(['a','b','c'],values);right=build(['b','a','c'],values);assert len(left.image_dictionary)==2 and len(right.image_dictionary)==1
positive=build(['a','b'],{'a':0,'b':0});assert len(positive.image_dictionary)==1 and len(next(iter(positive.image_dictionary.values())))==2
big=build(['a','far'],{'a':0,'far':(1<<64)-1});small=build(['small'],{'small':0});merged=big.merge_near_duplicate_dictionaries(small);assert len(merged)==2 and len(small.image_dictionary)==1 and small.image_dicionary is merged
E=runpy.run_path(str(b/'02-exact_duplicate.py'),run_name='owned')['ExactDuplicate']
with tempfile.TemporaryDirectory(prefix='image152-',dir='.dedup-work') as t:
 file=Path(t)/'owned';file.write_bytes(b'abc')
 try:E([]).create_image_hash(str(file));raise AssertionError('text accepted')
 except TypeError:pass
 file.write_bytes(b'\x89PNG\r\n\x1a\n')
 try:E([]).create_image_hash(str(file));raise AssertionError('binary accepted')
 except UnicodeDecodeError:pass
out={'controls_passed':5,'scope':'Original near-duplicate grouping methods AST unchanged with real Simhash2.1.2;features deliberately supplied as integer fingerprints;no Pillow feature extraction or actual image matching.Original exact module tested on Python3.14 compatibility only,not original Python2 runtime.','findings':['Chain fingerprints0,1,3 at k1 produce2 groups in A/B/C order but1 group in B/A/C order.','Equal fingerprint positive groups both records.','Merge assigns misspelled image_dicionary on smaller argument while image_dictionary remains stale.','Python3 text path raises TypeError hashing str.','Python3 PNGheader bytes fail UTF8 text decode.']};Path(__file__).with_name('results.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out))
