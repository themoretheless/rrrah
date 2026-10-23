import ast,contextlib,csv,io,json,tempfile
from pathlib import Path
root=Path('.dedup-work/examples088-20261024')
results=[]
for path in ['csv_example/csv_evaluation.py','patent_example/patent_evaluation.py','record_linkage_example/record_linkage_example_evaluation.py','gazetteer_example/gazetteer_evaluation.py']:
 tree=ast.parse((root/path).read_text()); selected=[n for n in tree.body if isinstance(n,(ast.Import,ast.ImportFrom,ast.FunctionDef))];ns={};exec(compile(ast.Module(body=selected,type_ignores=[]),path,'exec'),ns)
 errors=[]
 for found,truth in [(set(),{frozenset(('a','b'))}),({frozenset(('a','b'))},set())]:
  try:
   with contextlib.redirect_stdout(io.StringIO()):ns['evaluateDuplicates'](found,truth)
  except Exception as e:errors.append(type(e).__name__)
 entry={'path':path,'empty_found_or_truth_errors':errors}
 if 'linkPairs' in ns:
  with tempfile.TemporaryDirectory(prefix='rrrah-088-eval-') as td:
   fixture=Path(td)/'links.csv'
   with fixture.open('w') as f:
    w=csv.writer(f);w.writerow(['source file','Cluster ID']);w.writerows([['left','paired'],['right','paired'],['left','singleton'],['left','same-side'],['left','same-side']])
   pairs=ns['linkPairs'](str(fixture),'Cluster ID');entry['pairs']=sorted([sorted(p) for p in pairs],key=lambda x:(len(x),x));entry['expected_cross_source_pairs']=[[0,1]]
 results.append(entry)
assert all(r['empty_found_or_truth_errors']==['ZeroDivisionError','ZeroDivisionError'] for r in results)
assert results[2]['pairs']==[[2],[3],[4],[0,1]]
assert results[3]['pairs']==[[0,1]]
print(json.dumps({'scope':'Unmodified original import/function AST nodes, main statements excluded; finite owned CSV fixtures. No trained model or database invoked.','results':results},indent=2))
