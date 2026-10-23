"""Original branch-and-bound module definitions, exhaustive finite subset oracle."""
import ast,itertools,json,pathlib,random
p=pathlib.Path(__file__).resolve().parents[3]/'.dedup-work/dedupe083-20261024/dedupe/branch_and_bound.py'
t=ast.parse(p.read_bytes());t.body=[n for n in t.body if not(isinstance(n,ast.ImportFrom) and n.level)]
ns={'Predicate':object,'Cover':dict};exec(compile(ast.fix_missing_locations(t),str(p),'exec'),ns)
class P:
 def __init__(self,cost):self.cover_count=cost
rng=random.Random(83);cases=0
for _ in range(128):
 preds=[P(rng.randrange(1,10)) for i in range(5)]
 cover={p:frozenset(i for i in range(6) if rng.random()<0.5) for p in preds}
 reachable=set().union(*cover.values())
 for target in range(len(reachable)+1):
  subsets=[s for n in range(6) for s in itertools.combinations(preds,n) if len(set().union(*(cover[p] for p in s)))>=target]
  optimum=min(sum(p.cover_count for p in s) for s in subsets)
  got=ns['search'](cover,target,10000)
  assert len(set().union(*(cover[p] for p in got)))>=target
  assert sum(p.cover_count for p in got)==optimum
  cases+=1
print(json.dumps({'finite_cases_passed':cases,'predicates_per_case':5,'universe_size':6,'seed':83,'call_budget':10000,'scope':'Unchanged original search/helper definitions with synthetic predicate costs and exhaustive subset oracle. Does not qualify default2500 budget or learned predicate coverage.'},indent=2))
