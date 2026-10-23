"""Original receipt validator on owned synthetic inputs/outputs and aliases."""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
from tempfile import TemporaryDirectory
BASE=Path(__file__).resolve().parent
EV=BASE/'evidence/Hao0321--claude-skill-code-cleanup'
path=BASE.parents[2]/'.dedup-work/cleanup067-20261023/code-cleanup-helper/scripts/check_build_receipt.py'
data=path.read_bytes();pin=next(x for x in json.loads((EV/'tree.json').read_text())['tree'] if x['path']=='code-cleanup-helper/scripts/check_build_receipt.py')
assert hashlib.sha1(b'blob '+str(len(data)).encode()+b'\0'+data).hexdigest()==pin['sha']
spec=importlib.util.spec_from_file_location('rrrah_cleanup067_receipt',path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
m.run_self_test()
with TemporaryDirectory(prefix='rrrah-cleanup-receipt-') as directory:
    root=Path(directory);(root/'src').mkdir();(root/'dist').mkdir()
    source=root/'src/input.txt';source.write_bytes(b'x');(root/'dist/output.bin').write_bytes(b'y')
    valid=m.make_receipt(root);assert m.evaluate(valid,root)['status']=='GREEN'
    alias=root/'src/alias.txt';alias.symlink_to(source)
    linked=copy.deepcopy(valid);linked['inputs'][0]['path']='src/alias.txt';assert m.evaluate(linked,root)['status']=='GREEN'
    overlap=copy.deepcopy(valid);overlap['outputs']=[{'path':'src/alias.txt',**m.file_identity(source)}]
    assert m.evaluate(overlap,root)['status']=='GREEN'
    duplicate=copy.deepcopy(valid);duplicate['inputs'].append({**duplicate['inputs'][0],'path':'./src/input.txt'})
    assert m.evaluate(duplicate,root)['status']=='GREEN'
    boolean=copy.deepcopy(valid);boolean['schemaVersion']=True;boolean['inputs'][0]['bytes']=True
    assert m.evaluate(boolean,root)['status']=='GREEN'
    wrong=copy.deepcopy(valid);wrong['outputs'][0]['bytes']=2
    assert m.evaluate(wrong,root)['status']=='BLOCK'
    outside=root.parent/(root.name+'-absent')
    escape=root/'src/escape.txt';escape.symlink_to(outside)
    assert m._safe_path(root,'src/escape.txt')[1] is not None
print(json.dumps({'pinned_module_verified':True,'original_self_test_passed':True,'additional_controls_passed':7,'inside_symlink_accepted':True,'same_file_input_output_via_alias_accepted':True,'dot_alias_duplicate_accepted':True,'boolean_schema_and_size_accepted':True,'wrong_size_blocked':True,'outside_symlink_rejected':True,'real_build_or_package_proven':False},indent=2))
