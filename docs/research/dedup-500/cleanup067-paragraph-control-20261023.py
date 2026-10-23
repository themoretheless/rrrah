"""Selected original paragraph functions, no skill invocation or repository cleanup."""
import ast
import hashlib
import json
import re
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path
from tempfile import TemporaryDirectory
from typing import Any

BASE=Path(__file__).resolve().parent
EVIDENCE=BASE/'evidence/Hao0321--claude-skill-code-cleanup'
source=(EVIDENCE/'03-audit_core.py').read_bytes()
pin=next(e for e in json.loads((EVIDENCE/'tree.json').read_text())['tree'] if e['path']=='code-cleanup-helper/scripts/audit_core.py')
assert hashlib.sha1(b'blob '+str(len(source)).encode()+b'\0'+source).hexdigest()==pin['sha']
names={'Finding','read_text','rel','normalized_paragraphs','audit_duplicates'}
tree=ast.parse(source)
selected=[n for n in tree.body if isinstance(n,(ast.ClassDef,ast.FunctionDef)) and n.name in names]
assert len(selected)==len(names)
namespace=dict(globals())
exec(compile(ast.Module(body=selected,type_ignores=[]),'<pinned-paragraph-functions>','exec'),namespace)
config={'duplicate_min_chars':80,'duplicate_min_occurrences':3}
with TemporaryDirectory(prefix='rrrah-cleanup-paragraph-') as directory:
    root=Path(directory)
    def findings(payloads,suffix='.md'):
        paths=[]
        for i,payload in enumerate(payloads):
            path=root/f'{i}{suffix}';path.write_bytes(payload);paths.append(path)
        return namespace['audit_duplicates'](root,paths,config)
    repeated=b'A paragraph with enough letters to pass the configured length threshold and demonstrate text-only grouping.'
    assert findings([repeated]*2)[0].status=='PASS'
    assert findings([repeated]*3)[0].status=='FAIL'
    assert findings([repeated]*3,'.py')[0].status=='PASS'
    assert findings([b'```\n'+repeated]*3)[0].status=='PASS'
    invalid=[bytes([b])*90 for b in (0xff,0xfe,0xfd)]
    assert len({hashlib.sha256(b).digest() for b in invalid})==3
    replacement=findings(invalid)
    assert replacement[0].status=='FAIL'
    assert len(replacement[0].details['locations'])==3
    assert findings([repeated,repeated.lower(),repeated.upper()])[0].status=='FAIL'
print(json.dumps({'git_blob_verified':True,'selected_original_functions':sorted(names),'controls_passed':6,'two_file_duplicate_below_default_threshold':True,'three_file_repeat_detected':True,'python_text_excluded':True,'unterminated_fence_content_excluded':True,'different_invalid_utf8_bytes_grouped':True,'case_variants_grouped':True,'full_module_or_original_suite_run':False},indent=2))
