#!/usr/bin/env python3
"""Audit terminal atomic control; unknown never counts as a negative rejection."""
from pathlib import Path
import hashlib,json,math,shutil
from PIL import Image
R=Path(__file__).resolve().parents[1];D=R/'docs/research';P=D/'dedup-third-hard-negative-consensus-20261014.json';d=json.loads(P.read_text());sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert d['status']=='complete_pending_audit'and d['returncode']==0 and d['source_unchanged']and d['stderr']==''
assert all(sha(p)==h for p,h in d['pins'].items())
probe_path=D/'dedup-runner-native-probe-20261014.json';probe=json.loads(probe_path.read_text());assert probe['returncode']==0 and probe['source_unchanged']
assert all(sha(p)==h for p,h in probe['pins'].items())and sha(probe['log'])==probe['log_sha256']and sha(probe['executable'])==probe['executable_sha256']
manifest=Path(probe['source_manifest']);m=json.loads(manifest.read_text());source=Path(d['source']);assert str(source)==probe['source']and all(sha(source/p)==h for p,h in m['source_sha256'].items())
plan_path=D/'dedup-consensus-current-negative-plan-20261013.json';plan=json.loads(plan_path.read_text());case=plan['cases'][130];assert case==d['case']and case['index']==case['priority_rank']==130 and case['expected']=='different'and case['groundtruth']=='publisher_excluded_wrong_origin'
assert case['origin']!=case['right']and d['command']==[probe['executable'],*case['arguments']]
with Image.open(case['left'])as a,Image.open(case['right'])as b:ad=list(a.size);bd=list(b.size)
assert ad==case['source_dimensions']and bd==case['target_dimensions']
assert case['source_tolerance']==2*math.hypot(*ad)/math.hypot(*bd)and case['reverse_source_tolerance']==2*math.hypot(*bd)/math.hypot(*ad)
assert case['arguments'][3:]==['640',repr(case['source_tolerance']),'1000000',repr(case['reverse_source_tolerance']),'false']
result=json.loads(d['stdout']);assert result['support']is None and result['forward_support']is True and result['reverse_support']is None
assert result['forward_model_attempted']is True and result['reverse_model_attempted']is True and result['managed_used']==0 and result['managed_peak']==334746000 and result['managed_peak']<=512*1024*1024
policy=Path(probe['harness'])/'src/bin/native_bank_current_all_positives.rs';text=policy.read_text();assert 'tch::set_num_threads(2);'in text and 'tch::manual_seed(173);'in text and 'MemoryBudget::new(512 * 1024 * 1024)'in text
runtime_plan=D/'dedup-consensus-native-plan-20261013.json';runtime=json.loads(runtime_plan.read_text());runtime_pins={p:h for p,h in runtime['pins'].items()if p.endswith('.dylib')};assert len(runtime_pins)==7 and all(sha(p)==h for p,h in runtime_pins.items())
diagnostic=D/'dedup-third-hard-negative-diagnostic-20261014.json';old=json.loads(diagnostic.read_text());assert all(sha(R/p if not p.startswith('/')else p)==h for p,h in old['pins'].items())
F=D/'dedup-third-hard-negative-consensus-20261014';F.mkdir(exist_ok=False);(F/'stdout.json').write_text(d['stdout']);(F/'stderr.log').write_text(d['stderr']);assert json.loads((F/'stdout.json').read_text())==result
O=D/'dedup-third-hard-negative-consensus-audit-20261014.json';assert not O.exists();O.write_text(json.dumps(dict(status='verified_single_frozen_runner_hard_negative_unknown',report=str(P.relative_to(R)),report_sha256=sha(P),auditor_sha256=sha(__file__),source=str(source),source_manifest=str(manifest),source_manifest_sha256=sha(manifest),case_index=130,source_dimensions=ad,target_dimensions=bd,result=result,overall_unknown=1,confirmed_different_rejections=0,false_positive_supports=0,managed_peak_bytes=result['managed_peak'],managed_credit_released=True,observed_wall_seconds=d['elapsed_seconds'],raw_stdout=str((F/'stdout.json').relative_to(R)),raw_stdout_sha256=sha(F/'stdout.json'),runtime_after_pins=runtime_pins,runtime_capture_limit='Current post-run seven-dylib hashes match retained prior native plan; this producer did not separately capture seven runtime hashes at its own launch. Input/weights/source/probe pins are checked before/after.',historical_diagnostic=str(diagnostic.relative_to(R)),historical_diagnostic_sha256=sha(diagnostic),scope='Single actual atomic two-direction native/opposite/model25bank operation on frozen source6linear-runner, common normalized negative130policy. Forward locally supports, reverseunknown, overallunknown; no false support here but no confirmed rejection. Historical fixed2wrong-to-query local35inlier support used different source/policy/tolerances; no isolated causal AND-fix attribution. Not current unroll/collection/testcfg-root source, full233negative campaign/default/wholefile/universalprecision/recall/platform or timing speedup proof. Keep separate from pre-cache74negative prefix.'),indent=2)+'\n');print(O,result)
