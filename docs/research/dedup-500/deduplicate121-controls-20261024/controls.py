"""Execute original AST function bodies with real Shapely; vpype annotation stub only."""
import ast,json,types,sys
from pathlib import Path
import shapely
from shapely import union_all
from shapely.geometry import LineString,MultiLineString
from shapely.geometry.base import BaseGeometry
from shapely.strtree import STRtree
from typing import List,Tuple
source=Path('docs/research/dedup-500/evidence/LoicGoulefert--deduplicate/pinned/deduplicate/deduplicate.py')
tree=ast.parse(source.read_text()); functions=[n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name in ['_flatten_lines','_deduplicate_layer']]
ns=dict(List=List,Tuple=Tuple,BaseGeometry=BaseGeometry,LineString=LineString,MultiLineString=MultiLineString,STRtree=STRtree,union_all=union_all,vp=types.SimpleNamespace(LineCollection=list),tqdm=lambda x,**kw:x)
exec(compile(ast.Module(body=functions,type_ignores=[]),str(source),'exec'),ns)
f=ns['_deduplicate_layer'];line=[0j,10+0j];results=[]
for name,lines,tol,want_kept,want_removed in [('identical_positive',[line,line],0.01,10,10),('reversed_positive',[line,line[::-1]],0.01,10,10),('disjoint_positive',[line,[20+0j,30+0j]],0.01,20,0),('zero_tolerance_retains_identical_lines',[line,line],0,20,0)]:
 kept,removed,mask=f(lines,tol,True,True,[]);kl=sum(x.length for x in kept);rl=sum(x.length for x in removed);assert abs(kl-want_kept)<1e-8 and abs(rl-want_removed)<1e-8,(name,kl,rl);results.append(dict(name=name,kept_length=kl,removed_length=rl,mask_count=len(mask)))
out=dict(controls_passed=len(results),python=sys.version,shapely=shapely.__version__,scope='Original two AST functions;real native Shapely;vpype list annotation substitute;no full plugin CLI or upstream test suite',results=results)
Path(__file__).with_name('results.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out))
