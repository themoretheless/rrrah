#!/usr/bin/env python3
from pathlib import Path
import hashlib
import json
import statistics
import struct

R=Path(__file__).resolve().parents[1]
P=R/'docs/research/dedup-gradient-unroll4-natural-cpu-20261014.json'
d=json.loads(P.read_text())
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert d['status']=='completed_pending_audit_no_adoption'
assert len(d['cases'])==len(d['results'])==8
assert all(sha(p)==h for p,h in d['pins'].items())
assert all(sha(p)==h for p,h in d['feature_pins'].items())
rows=[]
for case,result in zip(d['cases'],d['results']):
    assert result['index']==case['index'] and result['label']==case['label']
    counts=[]
    for filename in case['inputs']:
        data=Path(filename).read_bytes();count=struct.unpack('<Q',data[:8])[0]
        assert len(data)==8+count*1040
        counts.append(count)
    assert result['bitwise_distance_pairs']==counts[0]*counts[1]
    timed=result['rows'];assert len(timed)==20 and len({r['checksum'] for r in timed})==1
    ratios=[]
    for pair in range(10):
        group=[r for r in timed if r['pair']==pair]
        order=['baseline','candidate'] if pair%2==0 else ['candidate','baseline']
        assert [r['version'] for r in group]==order
        assert all(r['cpu_seconds']>0 and r['wall_seconds']>0 for r in group)
        if pair:
            cpu={r['version']:r['cpu_seconds'] for r in group}
            ratios.append(cpu['candidate']/cpu['baseline'])
    assert ratios==result['ratios'] and statistics.median(ratios)==result['median_cpu_ratio']
    rows.append(dict(index=case['index'],label=case['label'],median_cpu_ratio=statistics.median(ratios),ratio_range=[min(ratios),max(ratios)],bitwise_distance_pairs=result['bitwise_distance_pairs']))
O=R/'docs/research/dedup-gradient-unroll4-natural-cpu-audit-20261014.json'
assert not O.exists()
O.write_text(json.dumps(dict(status='verified_eight_direction_natural_kernel_experiment_no_adoption',pins={str(p):sha(p) for p in [P,Path(__file__)]},rows=rows,bitwise_distance_pairs=sum(r['bitwise_distance_pairs'] for r in rows),unique_ordered_pairs=len({tuple(c['inputs']) for c in d['cases']}),scope=d['scope'],limits='CPU includes process startup/feature loading/teardown. No statistical significance, independent fresh feature extraction, full matcher or whole native operation claim.'),indent=2)+'\n')
print(rows)
