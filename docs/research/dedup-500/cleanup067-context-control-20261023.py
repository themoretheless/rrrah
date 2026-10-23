"""Original context-budget module with explicit synthetic counters, not real token counts."""
import copy
import hashlib
import importlib.util
import json
import sys
from pathlib import Path
from tempfile import TemporaryDirectory
BASE=Path(__file__).resolve().parent
EV=BASE/'evidence/Hao0321--claude-skill-code-cleanup'
path=BASE.parents[2]/'.dedup-work/cleanup067-20261023/code-cleanup-helper/scripts/check_context_budget.py'
data=path.read_bytes();pin=next(x for x in json.loads((EV/'tree.json').read_text())['tree'] if x['path']=='code-cleanup-helper/scripts/check_context_budget.py')
assert hashlib.sha1(b'blob '+str(len(data)).encode()+b'\0'+data).hexdigest()==pin['sha']
spec=importlib.util.spec_from_file_location('rrrah_cleanup067_context',path);m=importlib.util.module_from_spec(spec);sys.modules[spec.name]=m;spec.loader.exec_module(m)
with TemporaryDirectory(prefix='rrrah-cleanup-context-') as directory:
    root=Path(directory);(root/'one.md').write_text('data')
    manifest=m._test_manifest([m._test_route('one',['one.md'])]);counter=m._test_tokenizer()
    assert m.evaluate(manifest,root,tokenizer=counter)['status']=='GREEN'
    malformed=copy.deepcopy(manifest);malformed['routes'][0]['maxTokens']=True
    assert m.evaluate(malformed,root,tokenizer=counter)['status']=='BLOCK'
    unavailable=m.evaluate(manifest,root,tokenizer=m._test_tokenizer(False))
    assert unavailable['status']=='NOT_CHECKED' and m.exit_code(unavailable)==2
    (root/'alias.md').symlink_to(root/'one.md')
    linked=m._test_manifest([m._test_route('alias',['alias.md'])]);assert m.evaluate(linked,root,tokenizer=counter)['status']=='BLOCK'
    for value in ['./one.md','C:/one.md','../one.md']:
        assert m._safe_path(root,value)[1] is not None
    try:m._json_object([('a',1),('a',2)])
    except m.ContextBudgetError:pass
    else:raise AssertionError('duplicate JSON accepted')
    def mutate_after_read(text):
        (root/'one.md').write_text('new content')
        return len(text)
    changing=m.Tokenizer(True,'authored',None,mutate_after_read,'test-only-counter')
    result=m.evaluate(manifest,root,tokenizer=changing)
    assert result['status']=='GREEN'
    assert result['inventory'][0]['sha256']==hashlib.sha256(b'data').hexdigest()
    assert (root/'one.md').read_bytes()==b'new content'
print(json.dumps({'original_module_verified':True,'green_and_boolean_budget_checks_passed':True,'unavailable_tokenizer_blocks':True,'symlink_and_path_aliases_rejected':True,'duplicate_json_rejected':True,'mutation_after_read_not_revalidated':True,'real_tiktoken_or_token_quality_qualified':False},indent=2))
