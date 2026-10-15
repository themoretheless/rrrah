#!/usr/bin/env python3
from pathlib import Path
import ast,json,hashlib,types,sys
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';E=D/'evidence/idealo--imagededup';review=json.loads((D/'reviews/002.json').read_text());sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();pins={}
for name in ['02-hashing.py','04-bktree.py','05-test_bktree.py']:
 p=E/name;expected=next(v['sha256']for v in review['available_source_evidence']if v['local']==name);assert sha(p)==expected;pins[str(p.relative_to(R))]=expected
ns={'__name__':'isolated_pinned_bktree'};exec(compile((E/'04-bktree.py').read_text(),str(E/'04-bktree.py'),'exec'),ns)
h=ast.parse((E/'02-hashing.py').read_text());cls=next(n for n in h.body if isinstance(n,ast.ClassDef)and n.name=='Hashing');method=next(n for n in cls.body if isinstance(n,ast.FunctionDef)and n.name=='hamming_distance');module=ast.Module(body=[ast.ClassDef(name='Hashing',bases=[],keywords=[],body=[method],decorator_list=[])],type_ignores=[]);ast.fix_missing_locations(module);ns['np']=types.SimpleNamespace(sum=sum);exec(compile(module,'unchanged_hamming_method_with_scalar_sum_adapter','exec'),ns)
t=ast.parse((E/'05-test_bktree.py').read_text());removed=[];kept=[]
for node in t.body:
 if isinstance(node,ast.ImportFrom)and node.module.startswith('imagededup.'):
  removed.append(ast.unparse(node))
 else:kept.append(node)
t.body=kept;exec(compile(t,'unchanged_upstream_test_bodies_imports_only_removed','exec'),ns);names=sorted(k for k,v in ns.items()if k.startswith('test_')and callable(v))
for name in names:ns[name]()
BK=ns['BKTree'];distance=ns['Hashing'].hamming_distance;data={str(i):f'{i:016x}'for i in range(256)};tree=BK(data,distance);queries=0
for q in range(256):
 for radius in range(9):
  actual=dict(tree.search(f'{q:016x}',tol=radius));expected={str(i):bin(i^q).count('1')for i in range(256)if bin(i^q).count('1')<=radius};assert actual==expected;queries+=1
identical={str(i):'0'*16 for i in range(32)};duplicates=BK(identical,distance);assert len(duplicates.search('0'*16,tol=0))==32
try:BK({},distance)
except IndexError:empty='IndexError'
else:raise AssertionError('Expected observed empty input refusal')
limit=sys.getrecursionlimit()
try:BK({str(i):'0'*16 for i in range(limit+100)},distance)
except RecursionError:recursion='RecursionError'
else:raise AssertionError('Expected observed recursion limit')
a=(1<<64)|1;b=1<<63;wide=distance(hex(a),hex(b));assert wide==0 and bin(a^b).count('1')==3
O=D/'imagededup2-bktree-selected-runtime-v3-20261015.json';assert not O.exists();O.write_text(json.dumps(dict(status='verified_selected_source_controls_not_full_repository',repository=review['repository'],commit=review['commit'],producer_sha256=sha(__file__),source_pins=pins,upstream_test_bodies_passed=names,upstream_test_count=len(names),exhaustive_queries=queries,exhaustive_domain='All256eightbit patterns encoded fixed64bit hex, all radii0through8; every query compared against independent Python exact xor binary populationcount fullscan',equal_hash_zero_radius_result_count=32,negative_controls=dict(empty_mapping=empty,equal_hash_chain_recursion=recursion,python_recursion_limit=limit,oversized65bit_hashes_accepted_but_suffix_bit_lost=dict(left=hex(a),right=hex(b),observed=wide,independent_expected=3)),adapters=['Original BKTree and BkTreeNode module executed byte-for-byte','Original Hashing.hamming_distance AST method executed unchanged; np.sum adapter is Python builtin sum for boolean list, not numpy/package qualification','Only two imagededup package imports removed from upstream test AST; all test bodies and OrderedDict import unchanged'],scope='Selected complete138line BKTree module and167line testmodule read; hamming method only selected. No full idealo repository review, image decode/hash quality, numpy/CNN/multiprocessing/package execution, wholeRoot retrieval qualification or production code reuse.',findings=['Exact candidate completeness holds on exhaustivelytested fixed64bit domain','Empty direct BKTree input raises IndexError','Identical hashes form recursive zero-distance chain and exceed default Python recursion depth','Tolerance materializes2r+1integer list per visited node; static unbounded allocation risk, no huge allocation attempted','Hamming publicmethod does not reject >64bit inputs and zip can lose suffixbits; intended64bit domain remains separately tested']),indent=2)+'\n');print(len(names),'unchanged upstream test bodies;',queries,'exhaustive retrieval queries;3negativecontrols reproduced')
