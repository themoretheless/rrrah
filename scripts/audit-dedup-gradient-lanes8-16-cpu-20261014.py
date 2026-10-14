#!/usr/bin/env python3
from pathlib import Path
from collections import Counter
import hashlib,json,statistics,shutil
R=Path(__file__).resolve().parents[1];D=R/'docs/research';P=D/'dedup-gradient-lanes8-16-cpu-20261014.json';d=json.loads(P.read_text());sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert d['status']=='complete_pending_audit'and len(d['results'])==72
assert all(sha(p)==h for p,h in d['pins'].items())and all(sha(p)==h for p,h in d['input_pins'].items())
assert d['bitwise_controls']==dict(synthetic_distance_values=176128,natural_distance_values=8388608)
F=Path('/private/tmp/rrrah-gradient-lanes8-16-cpu-20261014');assert (F/'verify.stdout').read_text().strip()=='176128 8388608'and (F/'verify.stderr').read_bytes()==b''
positions={v:Counter()for v in ['4','8','16']};canonical=None
for group in range(24):
    order=['4','8','16'];offset=group%3;order=order[offset:]+order[:offset]
    if group%2:order=order[::-1]
    rows=[r for r in d['results']if r['group']==group];assert [r['version']for r in rows]==order
    for position,row in enumerate(rows):
        assert row['returncode']==0 and row['cpu_seconds']>0 and row['wall_seconds']>0
        assert sha(row['stdout'])==row['stdout_sha256']and sha(row['stderr'])==row['stderr_sha256']and Path(row['stderr']).read_bytes()==b''
        raw=Path(row['stdout']).read_bytes()
        if canonical is None:canonical=raw
        assert raw==canonical;int(raw)
        if group>=6:positions[row['version']][position]+=1
assert all(dict(c)=={0:6,1:6,2:6}for c in positions.values())
summary=[]
for version,declared in zip(['8','16'],d['summary']):
    ratios=[]
    for group in range(6,24):
        cpu={r['version']:r['cpu_seconds']for r in d['results']if r['group']==group};ratios.append(cpu[version]/cpu['4'])
    assert declared['version']==version and declared['ratios']==ratios and declared['median_cpu_ratio']==statistics.median(ratios)
    summary.append(dict(version=version,measured_groups=18,median_cpu_ratio=statistics.median(ratios),ratio_range=[min(ratios),max(ratios)]))
retained=D/'dedup-gradient-lanes8-16-cpu-20261014';retained.mkdir(exist_ok=False);pins={}
for p in F.iterdir():
    if p.is_file()and p.suffix in ['.rs','.log','.stdout','.stderr']:
        target=retained/p.name;shutil.copyfile(p,target);assert sha(p)==sha(target);pins[str(target.relative_to(R))]=sha(target)
O=D/'dedup-gradient-lanes8-16-cpu-audit-20261014.json';assert not O.exists();O.write_text(json.dumps(dict(status='verified_bounded_independent_lane_kernel_gain_pending_integration',report=str(P.relative_to(R)),report_sha256=sha(P),auditor_sha256=sha(__file__),fresh_processes=72,total_groups=24,warmup_groups=6,measured_groups=18,measured_position_counts={v:dict(c)for v,c in positions.items()},bitwise_controls=d['bitwise_controls'],summary=summary,retained_artifacts=pins,scope=d['scope'],next='Isolate16lane production matcher integration, preserve every nearest-table bit/cancellation/error/resource invariant, execute actual managed/cache/runner tests and public-API comparisons before adoption.'),indent=2)+'\n');print(summary)
