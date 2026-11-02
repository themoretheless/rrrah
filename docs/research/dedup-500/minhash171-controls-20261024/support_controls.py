"""Original support-compression/dominance helpers, finite exhaustive owned graphs.

No MinHash extraction, threshold matching, representative run(), corpus or IO.
"""
import ast
from collections import defaultdict
import __future__
import hashlib
import itertools
import json
from pathlib import Path

source=Path(__file__).resolve().parents[1]/'evidence/icodywu--minhash-dedup/02-minhash.py'
tree=ast.parse(source.read_text());cls=next(n for n in tree.body if isinstance(n,ast.ClassDef) and n.name=='SelectionStage')
names={'_compress_equal_support_keys','_reduce_support_dominated_keys','_rebuild_layers_from_key_weights'}
cls.body=[n for n in cls.body if isinstance(n,ast.FunctionDef) and n.name in names];cls.bases=[]
scope={'defaultdict':defaultdict}
exec(compile(ast.Module(body=[cls],type_ignores=[]),str(source),'exec',flags=__future__.annotations.compiler_flag),scope)
selection=scope['SelectionStage']()
edges=[list(c) for n in range(1,4) for c in itertools.combinations(range(3),n)]
checks=0
for mask in range(1<<len(edges)):
    original=[e[:] for i,e in enumerate(edges) if mask&(1<<i)]
    supports=defaultdict(list)
    for i,row in enumerate(original):
        for key in row:supports[key].append(i)
    reps={key:min(other for other in supports if supports[other]==support) for key,support in supports.items()}
    expected=[sorted({reps[k] for k in row}) for row in original]
    rows=[row[:] for row in original];aliases={}
    weights,alias_count=selection._compress_equal_support_keys(rows,aliases)
    assert [sorted(row or []) for row in rows]==expected,(mask,rows,expected)
    assert alias_count==sum(k!=rep for k,rep in reps.items())
    assert weights=={rep:len(supports[rep]) for rep in set(reps.values())}
    expected_aliases={rep:sorted(k for k in supports if reps[k]==rep and k!=rep) for rep in set(reps.values())}
    assert {k:sorted(v) for k,v in aliases.items()}=={k:v for k,v in expected_aliases.items() if v}
    # Complete original dominance helper, checking the finite coverage invariant.
    before=[row[:] if row is not None else None for row in rows]
    selection._reduce_support_dominated_keys(rows,weights)
    assert all(row is None or (row and set(row)<=set(old)) for old,row in zip(before,rows))
    survivors=set(weights)
    assert all(any(set(supports[rep])<=set(supports[key]) for rep in survivors) for key in reps if key==reps[key] and key not in survivors), (mask,original,rows,weights)
    checks+=1
chain=[[1,2],[2,3]];weights={1:1,2:2,3:1}
reduced=selection._reduce_support_dominated_keys(chain,weights)
assert reduced is None and weights=={1:1,2:2,3:1} and chain==[[1,2],[2,3]]
result={'scope':__doc__,'source_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),'authored_controls_run':2,'finite_graphs_checked':checks,'observations':['all128 hypergraphs on3 keys: original equal-support compression matches independent support-set reference; alias counts/weights correct; dominance leaves a surviving subset-support witness for each suppressed representative','chain support A:{0},B:{0,1},C:{1} is unchanged by dominance helper; it does not eliminate every support-dominated key and is not final run() clustering'],'original_tests_run':0,'rejected_control_assumption':'Initial assertion every bucket must remain nonempty was invalid: [[0],[0,1]] legitimately reduces to [None,[1]]. Raw failure retained in support-initial-failure.txt; not an upstream bug.'}
Path(__file__).with_name('support-results.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2))
