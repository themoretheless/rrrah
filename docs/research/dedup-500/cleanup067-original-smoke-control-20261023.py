"""Unchanged original smoke-test bodies with actual unchanged audit module.

Only import wiring is supplied; no mocks or external skill modules are run.
"""
import ast
import hashlib
import importlib.util
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
BASE=Path(__file__).resolve().parent
EV=BASE/'evidence/Hao0321--claude-skill-code-cleanup'
pins={x['path']:x['sha'] for x in json.loads((EV/'tree.json').read_text())['tree'] if x['type']=='blob'}
def verified(local,remote):
    data=(EV/local).read_bytes()
    assert hashlib.sha1(b'blob '+str(len(data)).encode()+b'\0'+data).hexdigest()==pins[remote]
    return data
core_path=EV/'03-audit_core.py';verified('03-audit_core.py','code-cleanup-helper/scripts/audit_core.py')
spec=importlib.util.spec_from_file_location('rrrah_cleanup067_smoke_core',core_path)
core=importlib.util.module_from_spec(spec);sys.modules[spec.name]=core;spec.loader.exec_module(core)
source=verified('06-self_test.py','code-cleanup-helper/scripts/self_test.py')
tree=ast.parse(source)
excluded={'test_public_sync_guard','test_sync_line_endings','test_json_contract'}
tests=[n.name for n in tree.body if isinstance(n,ast.FunctionDef) and n.name.startswith('test_') and n.name not in excluded]
selected=[n for n in tree.body if isinstance(n,ast.FunctionDef) and (n.name in tests or n.name in {'write','_single_update_finding'})]
namespace=dict(globals())
for name in ['collect_files','declared_versions','normalized_paragraphs','run_audit']:namespace[name]=getattr(core,name)
exec(compile(ast.Module(body=selected,type_ignores=[]),'<pinned-smoke-bodies>','exec'),namespace)
for name in tests:namespace[name]()
print(json.dumps({'original_unchanged_test_bodies_passed':len(tests),'names':tests,'excluded_tests':sorted(excluded),'full_original_module_or_aggregate_main_run':False,'real_core_module_used':True,'real_git_on_owned_fixture_only':True},indent=2))
