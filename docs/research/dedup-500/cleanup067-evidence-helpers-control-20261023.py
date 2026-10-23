"""Whole pinned pure helpers, authored metadata and owned paths only."""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
from tempfile import TemporaryDirectory
BASE=Path(__file__).resolve().parent
EV=BASE/'evidence/Hao0321--claude-skill-code-cleanup'
CACHE=BASE.parents[2]/'.dedup-work/cleanup067-20261023/run-benchmark-driven-rd/scripts'
pins={x['path']:x['sha'] for x in json.loads((EV/'tree.json').read_text())['tree'] if x['type']=='blob'}
def load(name):
    path=CACHE/(name+'.py');data=path.read_bytes()
    assert hashlib.sha1(b'blob '+str(len(data)).encode()+b'\0'+data).hexdigest()==pins['run-benchmark-driven-rd/scripts/'+name+'.py']
    spec=importlib.util.spec_from_file_location('rrrah_cleanup067_'+name,path)
    module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);return module
identity=load('evidence_identity');paths=load('cleanup_gate_paths');coverage=load('cleanup_gate_update_coverage')
valid={'bytes':1,'sha256':'a'*64}
assert identity.valid_identity(valid)
assert not identity.valid_identity({**valid,'bytes':True})
assert not identity.valid_identity({**valid,'bytes':0})
assert not identity.valid_identity({**valid,'sha256':'a'*63})
report,_=coverage.schema_12_fixture({'findings':[],'summary':{'review':0}})
assert coverage.validate_update_coverage(report,report['findings'])==[]
malformed=copy.deepcopy(report);malformed['update_coverage']['evidence']=['not-an-identity'];malformed['findings'][0]['details']=malformed['update_coverage']
assert coverage.validate_update_coverage(malformed,malformed['findings'])==[]
assert coverage.validate_update_coverage({'schema_version':'unsupported'},[])==[]
with TemporaryDirectory(prefix='rrrah-cleanup-paths-') as directory:
    root=Path(directory)/'target';root.mkdir()
    assert paths.relative_output_path(root/'report.json',root)=='report.json'
    assert paths.relative_output_path(Path(directory)/'outside.json',root) is None
assert paths.report_contains_path({'inventory':[{'path':'REPORT.JSON'}]},'report.json')
assert not paths.report_contains_path({'inventory':[{'path':'./report.json'}]},'report.json')
print(json.dumps({'pinned_modules_verified':3,'identity_positive_and_three_negative_checks_passed':True,'valid_update_schema_passed':True,'nested_evidence_not_validated_by_schema_helper':True,'unsupported_schema_deferred':True,'inside_outside_path_checks_passed':True,'inventory_matching_casefold_without_dot_normalization':True,'full_gate_or_real_evidence_validation_run':False},indent=2))
