"""Original pure predicate functions; key collisions are candidate behavior, not identity."""
import ast,itertools,json,pathlib
p=pathlib.Path(__file__).resolve().parents[3]/'.dedup-work/dedupe083-20261024/dedupe/predicate_functions.py'
t=ast.parse(p.read_bytes());names={'fingerprint','ngramsTokens','commonTwoElementsPredicate','wholeFieldPredicate'}
nodes=[n for n in t.body if isinstance(n,ast.FunctionDef) and n.name in names]
m=ast.Module(body=[ast.ImportFrom(module='__future__',names=[ast.alias(name='annotations')],level=0)]+nodes,type_ignores=[])
ns={};exec(compile(ast.fix_missing_locations(m),str(p),'exec'),ns)
assert ns['fingerprint']('ab c')==ns['fingerprint']('a bc')
assert ns['wholeFieldPredicate'](1)==ns['wholeFieldPredicate']('1')
a=['a','b','c'];b=['a','c','d']
assert len(set(a)&set(b))==2
assert not (ns['commonTwoElementsPredicate'](a)&ns['commonTwoElementsPredicate'](b))
cases=0
for length in range(8):
 words=[str(i) for i in range(length)]
 for n in range(1,5):
  expected=frozenset(' '.join(words[i:i+n]) for i in range(max(0,length-n+1)))
  assert ns['ngramsTokens'](words,n)==expected;cases+=1
print(json.dumps({'contiguous_gram_oracles':cases,'token_boundary_fingerprint_collision':True,'numeric_string_whole_field_collision':True,'two_shared_set_members_can_have_no_common_adjacent_pair_key':True,'scope':'Unchanged original pure predicate definitions. Observed collisions and omissions are candidate behavior; no classifier or exact-identity false acceptance claimed.'},indent=2))
