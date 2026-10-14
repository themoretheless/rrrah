#!/usr/bin/env python3
"""Audit retained raw parity outputs and instrumented work reports independently."""
from pathlib import Path
import hashlib,json,shutil
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
paths={k:D/f'dedup-gradient-topk-{k}-20261014.json' for k in ['experiment','stress','work']}
reports={k:json.loads(p.read_text())for k,p in paths.items()}
for k,d in reports.items():
    assert d['status'] in ['synthetic_correctness_passed','passed_pending_audit']
    assert all(sha(p)==h for p,h in d['pins'].items())
    assert all(sha(p)==h for p,h in d.get('feature_pins',{}).items())
initial=Path('/private/tmp/rrrah-gradient-topk-experiment-20261014/results.jsonl')
assert json.loads(initial.read_text())==dict(kind='correctness',fixtures=164,cancel_checkpoints_preserved=True)
stress=reports['stress'];assert len(stress['results'])==9
natural=0;synthetic=None
for i,row in enumerate(stress['results']):
    assert row['returncode']==0 and sha(row['stdout'])==row['stdout_sha256'] and sha(row['stderr'])==row['stderr_sha256']
    assert Path(row['stderr']).read_bytes()==b''
    values=[json.loads(line) for line in Path(row['stdout']).read_text().splitlines()]
    if i==0:
        assert values==[dict(kind='stress_parity',cases=2576,cancellations=8964,exact_nearest_bits=True,callback_counts=True)]
        synthetic=values[0]
    else:
        assert len(values)==2
        for distinct,value in zip([False,True],values):
            assert value==dict(kind='natural_parity',left=1024,right=1024,distinct=distinct,cancellations=3)
        natural+=2
stress_source=Path('/private/tmp/rrrah-gradient-topk-stress-20261014/stress.rs').read_text()
assert 'assert_eq!(expected,actual,"nearest/correspondence mismatch' in stress_source
assert 'assert_eq!(calls,count.get())' in stress_source
assert 'assert_eq!(before,count.get())' in stress_source
assert '(1..=calls).collect()' in stress_source
work=reports['work'];assert len(work['results'])==9
reductions=[]
for i,row in enumerate(work['results']):
    assert row['returncode']==0 and sha(row['stdout'])==row['stdout_sha256'] and sha(row['stderr'])==row['stderr_sha256']
    assert Path(row['stderr']).read_bytes()==b''
    values=[json.loads(line)for line in Path(row['stdout']).read_text().splitlines()];assert values==row['rows']
    assert [v['version'] for v in values]==[0,1,2]
    assert len({v['callbacks']for v in values})==1 and len({v['matches']for v in values})==1
    full,runner,top=values
    assert full['distance_pairs']==2*full['left']*full['right']
    assert top['distance_pairs']<=runner['distance_pairs']<=full['distance_pairs']
    assert top['distance_pairs']==top['left']*top['right']
    if i==0:assert full['left']==512 and top['distance_pairs']*2==runner['distance_pairs']
    else:reductions.append(1-top['distance_pairs']/runner['distance_pairs'])
F=D/'dedup-gradient-topk-controls-20261014';F.mkdir(exist_ok=False)
kept={}
for name,folder in [('initial','experiment'),('stress','stress'),('work','work')]:
    source=Path(f'/private/tmp/rrrah-gradient-topk-{folder}-20261014')
    destination=F/name;destination.mkdir()
    for p in source.iterdir():
        if p.is_file() and p.suffix in ['.rs','.log','.jsonl','.stderr']:
            target=destination/p.name;shutil.copyfile(p,target);assert sha(p)==sha(target);kept[str(target.relative_to(R))]=sha(target)
O=D/'dedup-gradient-topk-controls-audit-20261014.json';assert not O.exists()
O.write_text(json.dumps(dict(status='passed_bounded_standalone_controls',report_pins={str(p):sha(p)for p in paths.values()},auditor_sha256=sha(__file__),retained_artifacts=kept,initial_cases=164,stress_cases=2576,stress_cancellation_scenarios=8964,natural_directions=8,natural_modes=natural,natural_cancellation_scenarios=natural*3,natural_distance_work_reduction_vs_global_runner=dict(minimum=min(reductions),maximum=max(reductions)),correctness_reason='Every retained candidate is ordered by exact original distance. The first outside-radius candidate is a global minimum among eligible points because omitted distances cannot be smaller than any retained distance. If none qualifies the original full second pass remains. Strict nearest tie order and all original cancellation callbacks are retained.',limitations=['Standalone original pre-cache reference and shared kernel; not current managed-cache/native integration.','Prototype allocation uses unadmitted Vec scratch; production budget/fallible allocation and cancellation checkpoints in setup must be designed before adoption.','Reduced distance count does not prove lower CPU; list insertion overhead can erase savings.','No all-format/platform/default/collection precision or complete500source review claim.']),indent=2)+'\n')
print(O,min(reductions),max(reductions))
