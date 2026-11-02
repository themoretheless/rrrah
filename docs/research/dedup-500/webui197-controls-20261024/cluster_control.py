import ast,hashlib,itertools,json,logging,pathlib,typing
base=pathlib.Path(__file__).parent
source=base.parent/'evidence/WSH032--image-deduplicate-cluster-webui/03-deduplicate_fn.py'
raw=source.read_bytes();tree=ast.parse(raw)
fn=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='cluster_duplicates')
ns={'List':typing.List,'logging':logging,'IMAGEDEDUP_MODE_CHOICES_LIST':['Hash - recommend on CPU','CNN - recommend on GPU']}
class Hasher:
 def __init__(self,**kw):pass
 def find_duplicates(self,**kw):return ns['fixture']
ns['MyHasher']=Hasher
exec(compile(ast.Module(body=[fn],type_ignores=[]),str(source),'exec'),ns)
graph={'A':['B'],'B':['A','C'],'C':['B','D'],'D':['C']}
results=[]
for order in itertools.permutations(graph):
 ns['fixture']={k:graph[k] for k in order}
 groups=ns['cluster_duplicates']('/owned/synthetic',0,0,1,0,.9)
 sets=[set(g) for g in groups]
 results.append({'order':order,'groups':[sorted(g) for g in groups],'overlap':any(a&b for a,b in itertools.combinations(sets,2)),'single_component':len(sets)==1 and sets[0]==set(graph)})
assert any(x['overlap'] for x in results)
assert any(x['single_component'] for x in results)
report={'source_sha256':hashlib.sha256(raw).hexdigest(),'scope':'Original complete cluster_duplicates function AST-extracted unchanged; fake MyHasher supplies a symmetric synthetic graph. No imagededup/model/image/UI runtime or original tests executed.','cases':24,'overlapping_outputs':sum(x['overlap'] for x in results),'single_component_outputs':sum(x['single_component'] for x in results),'results':results}
(base/'cluster-control-results.json').write_text(json.dumps(report,indent=2)+'\n')
print({k:report[k] for k in ['cases','overlapping_outputs','single_component_outputs']})
