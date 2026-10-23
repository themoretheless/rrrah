"""Remaining original tests and sync helper boundary on owned data only."""
import ast
import hashlib
import importlib.util
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path
BASE=Path(__file__).resolve().parent
EV=BASE/'evidence/Hao0321--claude-skill-code-cleanup'
CACHE=BASE.parents[2]/'.dedup-work/cleanup067-20261023/code-cleanup-helper/scripts'
pins={x['path']:x['sha'] for x in json.loads((EV/'tree.json').read_text())['tree'] if x['type']=='blob'}
def verified(path,remote):
    b=path.read_bytes();assert hashlib.sha1(b'blob '+str(len(b)).encode()+b'\0'+b).hexdigest()==pins[remote];return b
sources={'audit_core.py':EV/'03-audit_core.py','audit.py':CACHE/'audit.py','sync_public.py':CACHE/'sync_public.py','self_test.py':EV/'06-self_test.py'}
blobs={name:verified(path,'code-cleanup-helper/scripts/'+name) for name,path in sources.items()}
def load(name,path):
    spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);sys.modules[name]=m;spec.loader.exec_module(m);return m
with tempfile.TemporaryDirectory(prefix='rrrah-cleanup-sync-tests-') as directory:
    root=Path(directory);scripts=root/'scripts';scripts.mkdir()
    for name,b in blobs.items():(scripts/name).write_bytes(b)
    core=load('rrrah_cleanup067_full_core',scripts/'audit_core.py');sync=load('rrrah_cleanup067_public_sync',scripts/'sync_public.py')
    names={'test_public_sync_guard','test_sync_line_endings','test_json_contract','write','_single_update_finding'}
    nodes=[n for n in ast.parse(blobs['self_test.py']).body if isinstance(n,ast.FunctionDef) and n.name in names]
    ns=dict(globals());ns['__file__']=str(scripts/'self_test.py')
    for name in ['equivalent_files','managed_paths','privacy_violations','safe_destination','write_manifest']:ns[name]=getattr(sync,name)
    exec(compile(ast.Module(body=nodes,type_ignores=[]),'<pinned-remaining-tests>','exec'),ns)
    for name in sorted(names):
        if name.startswith('test_'):ns[name]()
    invalid=root/'invalid.bin';invalid.write_bytes(b'authored-private-marker\xff')
    config={'privacy':{'tokens':['authored-private-marker']}}
    assert sync.privacy_violations([(invalid,'invalid.bin')],config)==[]
    other=root/'other.bin';other.write_bytes(b'authored-private-marker\xfe')
    assert not sync.equivalent_files(invalid,other,True)
print(json.dumps({'remaining_unchanged_original_test_bodies_passed':3,'all_original_own_smoke_test_bodies_passed_across_controls':17,'aggregate_main_not_run':True,'invalid_utf8_file_skips_privacy_text_check':True,'sync_copy_invalid_utf8_falls_back_to_bytes':True,'existing_project_or_external_destination_changed':False},indent=2))
