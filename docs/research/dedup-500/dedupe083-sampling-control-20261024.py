"""Unchanged original sampling functions, real NumPy, independent finite oracles."""
import ast,itertools,json,pathlib,random,warnings
import numpy
p=pathlib.Path(__file__).resolve().parents[3]/'.dedup-work/dedupe083-20261024/dedupe/convenience.py'
t=ast.parse(p.read_bytes());names={'randomPairs','randomPairsMatch','randomPairsWithReplacement'}
nodes=[x for x in t.body if isinstance(x,ast.FunctionDef) and x.name in names]
m=ast.Module(body=[ast.ImportFrom(module='__future__',names=[ast.alias(name='annotations')],level=0)]+nodes,type_ignores=[])
ns={'numpy':numpy,'random':random,'warnings':warnings};exec(compile(ast.fix_missing_locations(m),str(p),'exec'),ns)
random.seed(83);cases=0
for n in range(129):
 total=n*(n-1)//2
 for k in sorted({0,1,total//2,total,total+1}):
  got=[tuple(map(int,x)) for x in ns['randomPairs'](n,k)]
  assert len(got)==min(k,total) and len(set(got))==len(got)
  assert all(0<=a<b<n for a,b in got)
  if k>=total:assert set(got)==set(itertools.combinations(range(n),2))
  cases+=1
for a in range(1,17):
 for b in range(1,17):
  got=[tuple(map(int,x)) for x in ns['randomPairsMatch'](a,b,a*b)]
  assert len(got)==a*b and set(got)==set(itertools.product(range(a),range(b)))
  cases+=1
print(json.dumps({'finite_cases_passed':cases,'square_record_counts':[0,128],'rectangular_dimensions':[1,16],'random_seed':83,'scope':'Original sampling functions with NumPy and bounded independent combinations/product oracles. No huge integers, fallback replacement statistics or full training package qualification.'},indent=2))
