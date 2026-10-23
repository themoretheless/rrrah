"""Original revision-capture module with an authored local JSON producer."""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
from tempfile import TemporaryDirectory
BASE=Path(__file__).resolve().parent
EV=BASE/'evidence/Hao0321--claude-skill-code-cleanup'
path=BASE.parents[2]/'.dedup-work/cleanup067-20261023/run-benchmark-driven-rd/scripts/cleanup_gate_runtime.py'
data=path.read_bytes()
pin=next(e for e in json.loads((EV/'tree.json').read_text())['tree'] if e['path']=='run-benchmark-driven-rd/scripts/cleanup_gate_runtime.py')
assert hashlib.sha1(b'blob '+str(len(data)).encode()+b'\0'+data).hexdigest()==pin['sha']
spec=importlib.util.spec_from_file_location('rrrah_cleanup067_runtime',path)
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
with TemporaryDirectory(prefix='rrrah-cleanup-runtime-') as directory:
    root=Path(directory);provider=root/'provider';(provider/'scripts').mkdir(parents=True);skill=root/'skill';skill.mkdir()
    gate=provider/'scripts/check_skill_revision.py'
    snapshot={'algorithm':'skill-revision-sha256-v1','roots':1,'files':1,'bytes':0,'sha256':'a'*64}
    valid={'schemaVersion':1,'status':'CAPTURED','roots':[{'root':str(skill)}],'snapshot':snapshot}
    def produce(value,code=0):
        gate.write_text('import json,sys\nfrom pathlib import Path\nvalue='+repr(value)+'\nPath(sys.argv[sys.argv.index("--output")+1]).write_text(json.dumps(value))\nraise SystemExit('+str(code)+')\n')
    produce(valid)
    assert module.capture_skill_revision(provider,skill)==snapshot
    produce([])
    try:module.capture_skill_revision(provider,skill)
    except AttributeError:pass
    else:raise AssertionError('non-object did not expose unchecked get')
    malformed=copy.deepcopy(valid);malformed['roots']=[7];produce(malformed)
    try:module.capture_skill_revision(provider,skill)
    except AttributeError:pass
    else:raise AssertionError('non-object root did not expose unchecked get')
    inconsistent=copy.deepcopy(valid);inconsistent['snapshot'].update(roots=2,files=2);produce(inconsistent)
    assert module.capture_skill_revision(provider,skill)['roots']==2
    produce(valid)
    try:module.capture_skill_revision(provider,skill,'b'*64)
    except module.MeasurementError:pass
    else:raise AssertionError('hash mismatch accepted')
    produce(valid,1)
    try:module.capture_skill_revision(provider,skill)
    except module.MeasurementError:pass
    else:raise AssertionError('failed provider accepted')
print(json.dumps({'pinned_original_runtime_verified':True,'authored_local_producer_processes':6,'valid_contract_accepted':True,'non_object_report_untyped_error':True,'non_object_root_untyped_error':True,'snapshot_root_count_not_bound_to_root_list':True,'expected_hash_mismatch_rejected':True,'nonzero_exit_rejected':True,'actual_skill_revision_provider_run':False},indent=2))
