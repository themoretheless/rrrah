"""Selected original architecture pipeline on authored files; no skill invocation."""
import ast
import fnmatch
import hashlib
import json
from collections import Counter,defaultdict
from dataclasses import dataclass
from datetime import date
from pathlib import Path
from tempfile import TemporaryDirectory
from typing import Any,Iterable
BASE=Path(__file__).resolve().parent
EV=BASE/'evidence/Hao0321--claude-skill-code-cleanup'
source=(EV/'03-audit_core.py').read_bytes()
pin=next(e for e in json.loads((EV/'tree.json').read_text())['tree'] if e['path']=='code-cleanup-helper/scripts/audit_core.py')
assert hashlib.sha1(b'blob '+str(len(source)).encode()+b'\0'+source).hexdigest()==pin['sha']
names={'Finding','read_text','rel','_python_module_name','_match_any','_resolve_module','_resolve_import_candidate','_import_targets','_strongly_connected_components','_function_fingerprint','_layer_for','_parse_architecture_sources','_finalize_architecture_edges','_active_hotspot_exception','_dependency_findings','_active_function_exception','_function_findings','audit_architecture'}
tree=ast.parse(source)
selected=[n for n in tree.body if isinstance(n,(ast.FunctionDef,ast.ClassDef)) and n.name in names or isinstance(n,ast.Assign) and any(isinstance(t,ast.Name) and t.id=='ARCHITECTURE_SOURCE_LANGUAGES' for t in n.targets)]
assert len(selected)==len(names)+1
namespace=dict(globals())
exec(compile(ast.Module(body=selected,type_ignores=[]),'<pinned-architecture-definitions>','exec'),namespace)
with TemporaryDirectory(prefix='rrrah-cleanup-architecture-') as directory:
    root=Path(directory)
    def audit(files):
        paths=[]
        for name,text in files.items():
            p=root/name;p.write_text(text);paths.append(p)
        return namespace['audit_architecture'](root,paths,{'architecture':{}})
    result,details=audit({'bad.py':'def broken(:\n'})
    assert any(x.code=='architecture-parse-error' and x.status=='FAIL' for x in result)
    result,details=audit({'only.rs':'fn main() {}\n'})
    assert any(x.status=='NOT_CHECKED' for x in result)
    body=''.join(f'    a{i} = x + {i}\n' for i in range(8))+'    return x\n'
    result,details=audit({'left.py':'def first(x=1):\n'+body,'right.py':'def second(x=2):\n'+body})
    assert any(x.code=='duplicate-function-body' for x in result)
    result,details=audit({'a.py':'import b\n','b.py':'import a\n'})
    assert any(x.code=='dependency-cycle' for x in result)
    result,details=audit({'a.py':"__import__('b')\n",'b.py':"__import__('a')\n"})
    assert details['edges']==[]
    assert all(x.status=='PASS' for x in result)
print(json.dumps({'git_blob_verified':True,'selected_original_definitions':len(names),'controls_passed':5,'syntax_error_explicit_fail':True,'rust_only_not_checked':True,'different_defaults_reported_duplicate_under_default_size_thresholds':True,'static_import_cycle_reported':True,'dynamic_import_cycle_not_in_graph':True,'full_module_or_original_suite_run':False},indent=2))
