"""Whole pinned snapshot checker on synthetic reports; no outside project data."""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
BASE=Path(__file__).resolve().parent
EV=BASE/'evidence/Hao0321--claude-skill-code-cleanup'
p=EV/'04-check_audit_snapshot.py';data=p.read_bytes()
pin=next(e for e in json.loads((EV/'tree.json').read_text())['tree'] if e['path']=='code-cleanup-helper/scripts/check_audit_snapshot.py')
assert hashlib.sha1(b'blob '+str(len(data)).encode()+b'\0'+data).hexdigest()==pin['sha']
spec=importlib.util.spec_from_file_location('rrrah_cleanup067_snapshot',p)
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
module.run_self_test()
a=module.fixture_report([('one.txt',b'one\n')])
b=copy.deepcopy(a);a['target']='/authored/project-a';b['target']='/authored/project-b'
assert module.compare_reports(a,b)['status']=='FRESH'
b['config']={'different':True};b['mode']='architecture';b['summary']={'fail':999}
assert module.compare_reports(a,b)['status']=='FRESH'
boolean=copy.deepcopy(a);boolean['inventory'][0]['lines']=True;boolean['inventory'][0]['bytes']=False
assert module.inventory_map(boolean)['one.txt']['bytes'] is False
aliases=module.fixture_report([('one.txt',b'one\n'),('./one.txt',b'one\n')])
assert len(module.inventory_map(aliases))==2
case_duplicate=module.fixture_report([('one.txt',b'one\n'),('ONE.txt',b'one\n')])
try:module.inventory_map(case_duplicate)
except module.SnapshotError:pass
else:raise AssertionError('case duplicate accepted')
changed=module.fixture_report([('one.txt',b'two\n')])
assert module.compare_reports(a,changed)['changes']['changed']==['one.txt']
print(json.dumps({'git_blob_verified':True,'original_self_test_passed':True,'additional_controls_passed':6,'different_report_targets_ignored':True,'config_mode_findings_not_compared':True,'boolean_counts_accepted':True,'dot_path_aliases_accepted':True,'case_duplicate_rejected':True,'changed_content_metadata_detected':True,'real_filesystem_freshness_proven':False},indent=2))
