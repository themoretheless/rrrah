from pathlib import Path
import ast,os,hashlib,tempfile
source=Path('.dedup-work/plex352-20261021/DupCleanup.py').read_text()
tree=ast.parse(source);cls=next(n for n in tree.body if isinstance(n,ast.ClassDef) and n.name=='PlexDuplicateManager')
methods=[n for n in cls.body if isinstance(n,ast.FunctionDef) and n.name in ['get_file_hash','can_hardlink']]
module=ast.Module(body=[ast.ClassDef(name='Subject',bases=[],keywords=[],body=methods,decorator_list=[])],type_ignores=[])
ns={'os':os,'hashlib':hashlib};exec(compile(ast.fix_missing_locations(module),'original-selected-methods','exec'),ns)
s=ns['Subject']();s.log_message=lambda *a:None
with tempfile.TemporaryDirectory(prefix='rrrah-plex352-') as temp:
 root=Path(temp);a=root/'a';b=root/'b';other=root/'other';link=root/'hard'
 a.write_bytes(b'abcdef');b.write_bytes(b'abcdef');other.write_bytes(b'abcdeg');os.link(a,link)
 assert s.get_file_hash(a)==hashlib.sha256(b'abcdef').hexdigest();print('PASS original default chunk hash matches independent SHA256')
 assert s.can_hardlink(a,b)[0];print('PASS separate equal-content files admitted')
 assert not s.can_hardlink(a,other)[0];print('PASS same-size unequal content refused')
 assert not s.can_hardlink(a,link)[0];print('PASS already-hardlinked inode refused')
 assert s.get_file_hash(root/'missing') is None;print('PASS unavailable file returns None')
 assert s.get_file_hash(a,chunk_size=0)==hashlib.sha256(b'').hexdigest();print('PASS chunk_size=0 returns empty digest for nonempty file; invalid configuration not rejected')
print('RESULT 6 passed; original AST methods, real owned temporary files; no import-time dependency installer, GUI, Plex connection or hardlink conversion')
