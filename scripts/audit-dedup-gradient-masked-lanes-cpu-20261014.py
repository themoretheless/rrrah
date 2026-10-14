#!/usr/bin/env python3
from pathlib import Path
import hashlib,json,statistics,shutil
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
P=D/'dedup-gradient-masked-lanes-cpu-20261014.json';d=json.loads(P.read_text());assert d['status']=='complete_pending_audit'
assert all(sha(p)==h for p,h in d['pins'].items()) and all(sha(p)==h for p,h in d['feature_pins'].items())
assert len(d['results'])==3
summary=[]
for expected,case in zip(d['cases'],d['results']):
    assert case['label']==expected['label'] and case['inputs']==expected['inputs']
    rows=case['rows'];assert len(rows)==20
    canonical=None
    for i,row in enumerate(rows):
        pair=i//2;order=['runner','masked']if pair%2==0 else ['masked','runner'];assert row['pair']==pair and row['version']==order[i%2]
        assert row['command']==['/private/tmp/rrrah-gradient-masked-lanes-cpu-20261014/cpu',row['version'],*case['inputs']]
        assert row['returncode']==0 and row['cpu_seconds']>0 and row['wall_seconds']>0
        assert sha(row['stdout'])==row['stdout_sha256'] and sha(row['stderr'])==row['stderr_sha256']
        assert Path(row['stderr']).read_bytes()==b''
        raw=Path(row['stdout']).read_bytes()
        if canonical is None:canonical=raw
        assert raw==canonical
        lines=raw.decode().splitlines();assert int(lines[0])==len(lines)-1
        for line in lines[1:]:assert len(line.split())==4 and all(0<=int(v)<2**64 for v in line.split())
    ratios=[]
    for pair in range(1,10):
        cpu={r['version']:r['cpu_seconds']for r in rows if r['pair']==pair};ratios.append(cpu['masked']/cpu['runner'])
    assert ratios==case['ratios'] and statistics.median(ratios)==case['median_cpu_ratio']
    summary.append(dict(label=case['label'],median_cpu_ratio=case['median_cpu_ratio'],minimum_ratio=min(ratios),maximum_ratio=max(ratios),measured_pairs=9,matches=int(canonical.splitlines()[0])))
F=D/'dedup-gradient-masked-lanes-cpu-20261014';F.mkdir(exist_ok=False);retained={}
for p in Path('/private/tmp/rrrah-gradient-masked-lanes-cpu-20261014').iterdir():
    if p.is_file() and p.suffix in ['.rs','.log','.stdout','.stderr']:
        target=F/p.name;shutil.copyfile(p,target);assert sha(p)==sha(target);retained[str(target.relative_to(R))]=sha(target)
O=D/'dedup-gradient-masked-lanes-cpu-audit-20261014.json';assert not O.exists()
result=dict(status='passed_bounded_measurement_audit',report=str(P.relative_to(R)),report_sha256=sha(P),auditor_sha256=sha(__file__),fresh_processes=60,measured_pairs=27,matcher_repetitions_per_process=4,summary=summary,retained_artifacts=retained,scope=d['scope'])
O.write_text(json.dumps(result,indent=2)+'\n')
Decision=D/'dedup-gradient-masked-lanes-decision-20261014.json';assert not Decision.exists()
regression=any(s['median_cpu_ratio']>1.05 for s in summary if s['label'].startswith('natural_'))
benefit=any(s['median_cpu_ratio']<0.98 for s in summary if s['label'].startswith('natural_'))
Decision.write_text(json.dumps(dict(status='not_adopted_measured_regression' if regression else ('prototype_qualified_pending_managed_gate' if benefit else 'not_adopted_no_clear_cpu_benefit'),audit=str(O.relative_to(R)),audit_sha256=sha(O),production_changed=False,reason='Selective second-pass lanes require actual managed matcher integration, cancellation/admission and complete operation validation before adoption. Natural CPU regressions reject this prototype if present.',next_step='Use measured ratios to choose rejection or a temporary isolated current managed-library integration with all existing regression tests and alternating public-API comparison.',limits='Three bounded standalone cases, including one artificial clustered self bank; no universal, whole-operation or full-corpus speedup proof.'),indent=2)+'\n')
print(summary)
