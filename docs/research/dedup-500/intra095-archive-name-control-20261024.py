import ast,hashlib,io,json,sys,tempfile,types,warnings,zipfile
from pathlib import Path
base=Path(__file__).resolve().parent;sys.path.insert(0,str(base/'intra095-20261024'))
import scanner.hashFile as hf
class Reader:
 def __init__(self,path):self.archive=zipfile.ZipFile(path)
 def getFileList(self):return self.archive.namelist()
 def open(self,item):return self.archive.open(item)
source=base/'intra095-20261024/pArch.py';tree=ast.parse(source.read_text())
nodes=[n for n in tree.body if isinstance(n,(ast.ClassDef,ast.FunctionDef))]
env=dict(hf=hf,UniversalArchiveInterface=types.SimpleNamespace(ArchiveReader=Reader),magic=types.SimpleNamespace(from_buffer=lambda *a,**kw:'text/plain'))
exec(compile(ast.Module(body=nodes,type_ignores=[]),str(source),'exec'),env)
with tempfile.TemporaryDirectory(prefix='rrrah-intra095-owned-') as d:
 p=Path(d)/'duplicates.zip'
 with warnings.catch_warnings():
  warnings.simplefilter('ignore',UserWarning)
  with zipfile.ZipFile(p,'w') as z:
   z.writestr('same.txt',b'first');z.writestr('same.txt',b'other')
 arch=env['PhashArchive'](p);rows=list(arch.iterHashes());physical=[arch.archive.read(info) for info in arch.archive.infolist()]
 result=dict(physical_member_contents_distinct=len(set(physical))==2,yielded_count=len(rows),yielded_contents=[v['cont'].decode() for k,v in rows],duplicate_names_share_cached_info=rows[0][1] is rows[1][1])
 rows[0][1]['hexHash']='changed-by-caller'
 result['caller_mutation_changes_next_lookup']=arch.getHashInfo('same.txt')['hexHash']=='changed-by-caller'
 arch.archive.close()
 assert result['yielded_contents']==['other','other'] and result['duplicate_names_share_cached_info']
 result['scope']='Exact original PhashArchive/fix_mime definitions;actual owned ZIP with two same-name distinct members;Python zipfile reader adapter and fixed MIME adapter,not UniversalArchiveInterface runtime'
 print(json.dumps(result,sort_keys=True))
