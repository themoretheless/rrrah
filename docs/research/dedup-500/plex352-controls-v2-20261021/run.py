from pathlib import Path
import ast
from types import SimpleNamespace as N
source=Path('.dedup-work/plex352-20261021/DupCleanup.py').read_text();t=ast.parse(source);cls=next(x for x in t.body if isinstance(x,ast.ClassDef))
names={'find_duplicate_media','get_media_size','_has_matching_children'}
module=ast.Module(body=[ast.ClassDef(name='Subject',bases=[],keywords=[],body=[x for x in cls.body if isinstance(x,ast.FunctionDef) and x.name in names],decorator_list=[])],type_ignores=[])
ns={};exec(compile(ast.fix_missing_locations(module),'selected-original-methods','exec'),ns);s=ns['Subject']();s.log_message=lambda *a:None
media=lambda path:N(parts=[N(file=path,size=10)],videoResolution='1080',videoCodec='h264',bitrate=1)
a=N(title='Same title',media=[media('/a1'),media('/a2')]);b=N(title='Same title',media=[media('/b1'),media('/b2')])
library=N(type='movie',title='Movies',all=lambda:[a,b]);s.plex=N(library=N(sections=lambda:[library]))
r=s.find_duplicate_media();assert len(r['movies'])==1;assert all(x['file'].startswith('/b') for x in r['movies']['Same title'])
print('PASS distinct Plex items with same movie title overwrite earlier duplicate group')
a.title='Different title';r=s.find_duplicate_media();assert len(r['movies'])==2;print('PASS distinct titles preserve two groups')
m=N(parts=[N(file='/part1',size=10),N(file='/part2',size=20)],videoResolution='1080',videoCodec='h264',bitrate=1)
a.media=[m,media('/other')];library.all=lambda:[a];r=s.find_duplicate_media();v=r['movies']['Different title'][0];assert v['size']==30 and v['file']=='/part1';print('PASS multipart media aggregates all part sizes but retains first part path only')
class Var:
 def __init__(self,v):self.v=v
 def get(self):return self.v
s.filter_vars={col:Var('h265' if col=='Codec' else '') for col in ['Type','Resolution','Codec','Size','Path','Action']}
s._unfiltered_items=[{'parent':'p','id':'c','text':'Version 1','values':['Movie','1080','h264','1 GB','/a','KEEP']}]
assert s._has_matching_children('p','');print('PASS empty title filter admits parent via child even when child fails codec filter')
print('RESULT 4 passed; original AST methods with synthetic Plex metadata/filter variables; no GUI, account, deletion or filesystem conversion')
