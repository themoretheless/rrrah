import ast,hashlib,importlib.metadata,json,logging,sys
from pathlib import Path
base=Path(__file__).resolve().parent;src=base/'intra095-20261024';dep=base/'intra095-uar-dependency-20261024';sys.path[:0]=[str(src),str(dep)]
import pArch,magic
arch=pArch.PhashArchive(str(src/'Tests/test_ptree_base/z_reg_junk.zip'))
try:info=arch.getHashInfo('Thumbs.db')
finally:arch.close()
tree=ast.parse((src/'deduplicator/ProcessArchive.py').read_text());cls=next(n for n in tree.body if isinstance(n,ast.ClassDef) and n.name=='ArchChecker');method=next(n for n in cls.body if isinstance(n,ast.FunctionDef) and n.name=='_shouldSkipFile');env={};exec(compile(ast.Module(body=[method],type_ignores=[]),'original-ProcessArchive.py','exec'),env)
owner=type('Owner',(),{'log':logging.getLogger('owned')})();skip=env['_shouldSkipFile'](owner,'Thumbs.db',info['type']);assert info['type']=='application/x-ole-storage' and skip is False
print(json.dumps(dict(mime=info['type'],original_skip_result=skip,libmagic_version=magic.version(),thumbs_sha256=hashlib.sha256(info['cont']).hexdigest(),packages={p:importlib.metadata.version(p) for p in ['python-magic','pylzma','rarfile']},scope='Real original PhashArchive plus pinned UniversalArchiveInterface and native libmagic;exact original skip method body;owned original archive readonly,no DB or deletion'),sort_keys=True))
