#!/usr/bin/env python3
from pathlib import Path
import hashlib
import json
import re
import statistics

R=Path(__file__).resolve().parents[1];D=R/'docs/research'
P=D/'dedup-unroll4-native-operation-20261014.json';d=json.loads(P.read_text());sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert d['status']=='complete_pending_audit' and len(d['results'])==12 and len(d['builds'])==2
Q=Path(d['plan']);assert sha(Q)==d['plan_sha256'];q=json.loads(Q.read_text());assert all(sha(p)==h for p,h in q['pins'].items())
assert sha(R/'scripts/run-dedup-unroll4-native-operation-queue-20261014.py')==d['producer_sha256']
prior=R/q['required_first_audit'];assert sha(prior)==d['first_audit_sha256']
runtime=D/'dedup-unroll4-native-operation-runtime-before-20261014.json';rt=json.loads(runtime.read_text());assert all(sha(p)==h for p,h in rt['pins'].items())
manifests={}
for name,version in q['versions'].items():
    M=Path(version['source_manifest']);assert sha(M)==version['source_manifest_sha256'];m=json.loads(M.read_text());manifests[name]=m
    assert all(sha(Path(m['source'])/p)==h for p,h in m['source_sha256'].items())
    assert all(sha(Path(version['harness'])/p)==h for p,h in version['harness_pins'].items())
changed={p for p,h in manifests['baseline']['source_sha256'].items() if manifests['candidate']['source_sha256'].get(p)!=h}
assert changed=={'crates/rrrah-dedup/src/gradient.rs','crates/rrrah-dedup/src/gradient_matching_tests.rs'}
builds={row['version']:row for row in d['builds']};assert set(builds)=={'baseline','candidate'}
for row in builds.values():assert sha(row['executable'])==row['executable_sha256'] and sha(row['log'])==row['log_sha256']
records=[];case_metrics=[]
for case in q['cases']:
    expected=None;ratios=[]
    for pair in range(3):
        rows=[row for row in d['results'] if row['case']==case['label'] and row['pair']==pair]
        assert [row['version'] for row in rows]==(['baseline','candidate'] if pair%2==0 else ['candidate','baseline'])
        cpu={}
        for row in rows:
            assert sha(row['report'])==row['report_sha256'];raw=json.loads(Path(row['report']).read_text());assert raw['returncode']==0
            assert raw['command']==['/usr/bin/time','-l',builds[row['version']]['executable'],*case['arguments']]
            value=json.loads(raw['stdout']);assert value['support'] is True and value['forward_support'] is True and value['reverse_support'] is True and value['managed_used']==0
            if expected is None:expected=value
            else:assert value==expected
            assert row['managed_peak']==value['managed_peak'] and row['cpu_seconds']>0 and row['wall_seconds']>0
            timing=re.search(r'([\d.]+)\s+real\s+([\d.]+)\s+user\s+([\d.]+)\s+sys',raw['stderr']);assert timing
            measured=float(timing[2])+float(timing[3]);assert abs(measured-row['cpu_seconds'])<max(0.05,row['cpu_seconds']*0.01)
            cpu[row['version']]=row['cpu_seconds'];records.append(row)
        ratios.append(cpu['candidate']/cpu['baseline'])
    case_metrics.append(dict(case=case['label'],cpu_ratios=ratios,median_cpu_ratio=statistics.median(ratios),managed_peak=expected['managed_peak'],model_attempted=[expected['forward_model_attempted'],expected['reverse_model_attempted']]))
assert len(records)==12
O=D/'dedup-unroll4-native-operation-audit-20261014.json';assert not O.exists();O.write_text(json.dumps(dict(status='verified_two_control_full_operation_version_comparison',cases=case_metrics,rows=records,pins={str(p):sha(p) for p in [P,Q,prior,runtime,Path(__file__)]},scope=q['scope']+' All twelve full-operation outputs agree; source/harness/executable/inputs and seven shared runtime libraries rechecked. CPU crosschecked against time user+sys. Development positives only; no whole-corpus, default/UI, universal speedup or library completion claim.'),indent=2)+'\n');print(case_metrics)
