import ast,hashlib,json,sys,tempfile,types,warnings,zipfile
from pathlib import Path
import py7zlib
base=Path(__file__).resolve().parent;src=base/'intra095-20261024';dep=base/'intra095-uar-dependency-20261024';sys.path.insert(0,str(src))
import scanner.hashFile as hf
p=src/'Tests/testArches/testArches.7z';members=[]
with p.open('rb') as fp:
 arch=py7zlib.Archive7z(fp)
 for name in sorted(arch.getnames()):
  member=arch.getmember(name);content=member.read();member.checkcrc()
  members.append(dict(name=name,bytes=len(content),sha256=hashlib.sha256(content).hexdigest(),hash_info=hf.getHashDict(name,content)))
with zipfile.ZipFile(src/'Tests/testArches/testArches.zip') as z:
 equivalent=all(z.read(m['name']) and hashlib.sha256(z.read(m['name'])).hexdigest()==m['sha256'] for m in members)
assert equivalent and len(members)==6
source=dep/'UniversalArchiveInterface/__init__.py';tree=ast.parse(source.read_text());cls=next(n for n in tree.body if isinstance(n,ast.ClassDef) and n.name=='ArchiveReader');method=next(n for n in cls.body if isinstance(n,ast.FunctionDef) and n.name=='_getZipFileList');env={};exec(compile(ast.Module(body=[method],type_ignores=[]),str(source),'exec'),env)
with tempfile.TemporaryDirectory(prefix='rrrah-intra095-owned-') as d:
 p=Path(d)/'duplicate.zip'
 with warnings.catch_warnings():
  warnings.simplefilter('ignore',UserWarning)
  with zipfile.ZipFile(p,'w') as z:z.writestr('same.txt',b'first');z.writestr('same.txt',b'other')
 with zipfile.ZipFile(p) as z:
  names=env['_getZipFileList'](types.SimpleNamespace(archHandle=z))
  duplicate=dict(physical_members=len(z.infolist()),visible_names=names,visible_contents=[z.read(n).decode() for n in names],scanner_name_uniqueness_check_passes=len(names)==len(set(names)))
assert duplicate['physical_members']==2 and duplicate['visible_names']==['same.txt']
print(json.dumps(dict(sevenz_members=members,sevenzip_payloads_equal_zip=equivalent,duplicate_name_case=duplicate,scope='Real pylzma/py7zlib backend and original hashFile on original7z;exact AST-selected dependency ZIP listing method on owned ZIP;full UniversalArchiveInterface import still lacks native libmagic'),sort_keys=True))
