"""Original score/body ID conversion with NumPy and explicit score adapters."""
import ast,hashlib,json,pathlib
import numpy
p=pathlib.Path(__file__).resolve().parents[3]/'.dedup-work/dedupe083-20261024/dedupe/core.py'
b=p.read_bytes();tree=ast.parse(b)
fn=next(n for n in reversed(tree.body) if isinstance(n,ast.FunctionDef) and n.name=='sniff_id_type')
cl=next(n for n in tree.body if isinstance(n,ast.ClassDef) and n.name=='ScoreGazette')
m=ast.Module(body=[ast.ImportFrom(module='__future__',names=[ast.alias(name='annotations')],level=0),fn,cl],type_ignores=[])
ns={'numpy':numpy};exec(compile(ast.fix_missing_locations(m),str(p),'exec'),ns)
class Classifier:
 def predict_proba(self,features):return numpy.tile([0.1,0.9],(len(features),1))
score=ns['ScoreGazette'](lambda records:numpy.zeros((len(records),1)),Classifier())
normal=[((1,{}),(2,{})),((1,{}),(3,{}))]
a=score(normal);assert a['pairs'].tolist()==[[1,2],[1,3]]
prefix='x'*256
long=[((prefix+'a',{}),('right',{})),((prefix+'b',{}),('right',{}))]
a=score(long);assert a['pairs'][0,0]==a['pairs'][1,0]==prefix
assert long[0][0][0]!=long[1][0][0]
print(json.dumps({'source_sha256':hashlib.sha256(b).hexdigest(),'integer_ids_preserved':True,'distinct_257_character_ids_collapse_to_same_256_character_id':True,'scope':'Original ScoreGazette and sniff_id_type definitions with real NumPy, synthetic classifier/featurizer. No learned-model/full-package or arbitrary identifier qualification.'},indent=2))
