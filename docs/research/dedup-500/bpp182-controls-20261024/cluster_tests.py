"""Six unchanged original test bodies with original AST hash/global-cluster functions.

No cv2/imported application graph; module namespace shim and logger adapter only.
"""
import ast,json,logging,sys,types
from pathlib import Path
b=Path(__file__).resolve().parent;e=b.parent/'evidence/Arkalogy--best-photo-picker'
def functions(path,names,env):
 t=ast.parse(path.read_text());nodes=[ast.ImportFrom(module='__future__',names=[ast.alias(name='annotations')],level=0)]+[x for x in t.body if isinstance(x,ast.FunctionDef) and x.name in names];exec(compile(ast.fix_missing_locations(ast.Module(body=nodes,type_ignores=[])),str(path),'exec'),env)
env={'log':logging.getLogger('original-cluster-tests')};functions(e/'05-phash.py',{'hamming_distance','dual_hash_distance'},env);functions(e/'remaining-pinned/bpp/dedupe/cluster.py',{'_hash_distance','_global_dedup'},env)
m=types.ModuleType('bpp.dedupe.cluster');m.__dict__.update(env);sys.modules['bpp.dedupe.cluster']=m
s=e/'06-test_dedupe_phash.py';tree=ast.parse(s.read_text());nodes=[x for x in tree.body if (isinstance(x,ast.FunctionDef) and x.name in {'test_hamming_distance_symmetric','test_hamming_distance_known'}) or isinstance(x,ast.ClassDef)];exec(compile(ast.fix_missing_locations(ast.Module(body=nodes,type_ignores=[])),str(s),'exec'),env)
executed=[]
for n in ['test_hamming_distance_symmetric','test_hamming_distance_known']:env[n]();executed.append(n)
instance=env['TestGlobalDedupMonthProtection']()
for n in sorted(x for x in dir(instance) if x.startswith('test_')):getattr(instance,n)();executed.append(n)
# Counterexample uses same originals; first/high score monthly representative retained.
rows=[{'filepath':'high.jpg','phash':-1,'ahash':None,'aggregate_score':9},{'filepath':'low.jpg','phash':0,'ahash':None,'aggregate_score':1}];got=env['_global_dedup'](rows,8);assert [x['filepath'] for x in got]==['high.jpg']
p={'scope':__doc__,'original_test_bodies_passed':len(executed),'executed_names':executed,'authored_controls_passed':1,'counterexample':{'input_hashes':[-1,0],'fixed_width_distance':64,'threshold':8,'kept':[x['filepath'] for x in got]},'limits':'Three cv2-dependent original tests not executed; no full module/application/CLI runtime.'};(b/'cluster-test-results.json').write_text(json.dumps(p,indent=2)+'\n');print(json.dumps(p,indent=2))
