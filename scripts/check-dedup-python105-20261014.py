#!/usr/bin/env python3
"""Original function AST controls; no upstream startup, deletion or private paths."""
from pathlib import Path
import ast
from collections import defaultdict
import hashlib
import json
import os
import tempfile
import threading
import types

R=Path(__file__).resolve().parents[1]
E=R/'docs/research/dedup-500/evidence/sunhanaix--dedup_files'
OUT=R/'docs/research/dedup-python105-controls-20261014.json'
assert not OUT.exists()
names={'LICENSE':'01-LICENSE','README.md':'00-README.md','dedup_files.py':'02-dedup_files.py','util.py':'03-util.py'}
for item in json.loads((E/'tree.json').read_text())['tree']:
    b=(E/names[item['path']]).read_bytes();assert hashlib.sha1(b'blob '+str(len(b)).encode()+b'\0'+b).hexdigest()==item['sha']

def functions(file,namespace):
    tree=ast.parse(file.read_text());nodes=[node for node in tree.body if isinstance(node,ast.FunctionDef)]
    exec(compile(ast.Module(body=nodes,type_ignores=[]),str(file),'exec'),namespace)

utility={'os':os,'hashlib':hashlib}
functions(E/'03-util.py',utility)
errors=[]
logger=types.SimpleNamespace(error=errors.append)
namespace={'os':os,'ast':ast,'defaultdict':defaultdict,'util':types.SimpleNamespace(md5_file=utility['md5_file']),'mylog':logger,'md5_lock':threading.Lock(),'ino_lock':threading.Lock(),'array_lock':threading.Lock()}
functions(E/'02-dedup_files.py',namespace)
checks=[]

def check(name,value):
    assert value,name
    checks.append(name)

with tempfile.TemporaryDirectory(prefix='rrrah-python105-',dir='/private/tmp') as temporary:
    root=Path(temporary);a=root/'a';b=root/'b';link=root/'link';a.write_bytes(b'first');b.write_bytes(b'first');os.link(a,link)
    namespace['cfg']={'cache_file':str(root/'cache')}
    md5=utility['md5_file']
    check('real_md5_matches_standard_hash',md5(a)==hashlib.md5(b'first').hexdigest())
    check('equal_content_copies_same_hash',md5(a)==md5(b))
    check('hardlink_inode_comparison_true',utility['cmp_files'](a,link))
    check('equal_content_distinct_copies_inode_comparison_false',not utility['cmp_files'](a,b))
    real_os=utility['os'];utility['os']=types.SimpleNamespace(stat=lambda p:types.SimpleNamespace(st_size=5,st_ino=7,st_dev=1 if p=='a' else 2))
    check('same_inode_different_devices_misidentified_as_same_file',utility['cmp_files']('a','b'))
    utility['os']=real_os
    record={'file_path':str(a),'size':5,'md5':md5(a),'ino':os.stat(a).st_ino}
    namespace['md5_array']=[record];a.write_bytes(b'changed content')
    check('changed_file_keeps_path_only_cache_hit',namespace['find_record_by_file_path'](str(a)) is True)
    a.unlink()
    check('deleted_file_keeps_path_only_cache_hit',namespace['find_record_by_file_path'](str(a)) is True)
    a.write_bytes(b'first')
    grouped=namespace['convert_array_to_ino_dict']([dict(record,ino=7,device=1),dict(record,ino=7,device=2)])
    check('inode_dictionary_groups_different_devices',len(grouped[7])==2)
    namespace['write_cache_file']([record]);check('repr_cache_roundtrip',namespace['read_cache_file']()==[record])
    with (root/'cache').open('a') as stream:stream.write('{broken\n')
    try:namespace['read_cache_file']()
    except (SyntaxError,ValueError):check('truncated_cache_raises_instead_of_retaining_valid_prefix',True)
    else:raise AssertionError('cache parse should fail')
    check('dictionary_array_conversion_returns_nested_lists',namespace['convert_dict_to_array']({'x':[1,2]})==[[1,2]])
    def calculate(path,ino=None):
        hashes=defaultdict(list);inodes=defaultdict(list) if ino is None else ino;records=[]
        result=namespace['calculate_md5'](str(path),hashes,inodes,records)
        return result,hashes,inodes,records
    result,hashes,inodes,records=calculate(a)
    check('original_calculate_records_real_file',result is None and len(records)==1 and records[0]['md5']==md5(a))
    result,hashes,inodes,records=calculate(root/'missing')
    check('hash_failure_is_logged_and_swallowed',result is None and not records and errors)
    namespace['util']=types.SimpleNamespace(md5_file=lambda path:'forced_same_hash')
    hashes=defaultdict(list);inodes=defaultdict(list);records=[]
    b.write_bytes(b'different bytes')
    namespace['calculate_md5'](str(a),hashes,inodes,records);namespace['calculate_md5'](str(b),hashes,inodes,records)
    check('injected_hash_collision_groups_distinct_bytes_without_confirmation',len(hashes['forced_same_hash'])==2 and a.read_bytes()!=b.read_bytes())
    def changed_during_hash(path):
        old_hash=md5(path);Path(path).write_bytes(b'new longer bytes');return old_hash
    namespace['util']=types.SimpleNamespace(md5_file=changed_during_hash)
    expected=md5(a);_,_,_,records=calculate(a)
    check('old_hash_published_with_post_mutation_size',records[0]['md5']==expected and records[0]['size']==len(b'new longer bytes') and records[0]['md5']!=md5(a))
    namespace['util']=types.SimpleNamespace(md5_file=md5)
    barrier=threading.Barrier(2)
    class CoordinatedInodes(defaultdict):
        def __contains__(self,key):
            observed=super().__contains__(key);barrier.wait(timeout=10);return observed
    inodes=CoordinatedInodes(list);hashes=defaultdict(list);records=[]
    threads=[threading.Thread(target=namespace['calculate_md5'],args=(str(a),hashes,inodes,records)) for _ in range(2)]
    for thread in threads:thread.start()
    for thread in threads:thread.join(timeout=15);assert not thread.is_alive()
    check('coordinated_membership_race_records_same_inode_twice',len(records)==2 and len(inodes[os.stat(a).st_ino])==2)
OUT.write_text(json.dumps(dict(status='sixteen_original_function_controls_confirmed',count=len(checks),checks=checks,repository='sunhanaix/dedup_files',commit='6aa92d34f28654b9b942b8822634ebca2670be1b',pins={str(E/local):hashlib.sha256((E/local).read_bytes()).hexdigest() for local in names.values()},scope='Original function ASTs compiled unchanged; upstream module startup, registry, scan-all-directories, deletion/link replacement never executed. Real temporary files/MD5 plus explicit hash collision, cross-device stat and coordinated dictionary adapters. Not actual MD5 collision, Windows filesystem/platform qualification or probabilistic concurrency stress.'),indent=2)+'\n')
assert len(checks)==16
print(f'{len(checks)} controls confirmed')
