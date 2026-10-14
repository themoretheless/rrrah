#!/usr/bin/env python3
from pathlib import Path
import hashlib
import json
import statistics
import struct

R=Path(__file__).resolve().parents[1];D=R/'docs/research'
P=D/'dedup-gradient-masked-lanes-managed-benchmark-20261014.json';d=json.loads(P.read_text())
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert d['status']=='complete_pending_audit'
assert all(sha(p)==h for p,h in d['pins'].items())
assert all(sha(p)==h for p,h in d['input_pins'].items())
assert all(sha(p)==h for p,h in d['dependency_pins'].items())
assert len(d['cases'])==2 and len(d['results'])==4
folder=Path('/private/tmp/rrrah-gradient-masked-lanes-managed-benchmark-20261014')
records=[];coordinates={}
for case in d['cases']:
    counts=[]
    for filename in case['inputs']:
        data=Path(filename).read_bytes();count=struct.unpack('<Q',data[:8])[0];assert len(data)==8+count*1040;counts.append(count)
    base=(counts[0]+counts[1])*24+min(counts)*32
    for extra in [0,16*1024*1024]:
        matches=[row for row in d['results'] if row['case']==case['label'] and row['extra_bytes']==extra];assert len(matches)==1
        result=matches[0];expected=folder/(case['label']+'-'+str(extra)+'-expected.txt');text=expected.read_text();lines=text.splitlines();matched,peak=map(int,lines[0].split())
        assert matched==result['matches'] and peak==result['peak_bytes'] and len(lines)==matched+1
        values=[list(map(int,line.split())) for line in lines[1:]];assert all(len(value)==4 for value in values)
        if case['label'] in coordinates:assert values==coordinates[case['label']]
        else:coordinates[case['label']]=values
        assert peak<=base+extra
        if extra==0:assert peak==base
        elif counts[0]*counts[1]*8<=16*1024*1024:assert peak==base+counts[0]*counts[1]*8
        else:assert peak==base+(counts[0]+counts[1])*16
        timed=result['rows'];assert len(timed)==20 and all(row['output_sha256']==sha(expected) for row in timed)
        for row in timed:
            assert sha(row['stdout'])==row['stdout_sha256'] and sha(row['stderr'])==row['stderr_sha256']
            assert Path(row['stdout']).read_text()==text and Path(row['stderr']).read_bytes()==b''
        ratios=[]
        for pair in range(10):
            group=[row for row in timed if row['pair']==pair];assert [row['version'] for row in group]==(['baseline','candidate'] if pair%2==0 else ['candidate','baseline'])
            assert all(row['cpu_seconds']>0 and row['wall_seconds']>0 for row in group)
            if pair:
                cpu={row['version']:row['cpu_seconds'] for row in group};ratios.append(cpu['candidate']/cpu['baseline'])
        assert ratios==result['ratios'] and statistics.median(ratios)==result['median_cpu_ratio']
        records.append(dict(case=case['label'],extra_bytes=extra,matches=matched,peak_bytes=peak,median_cpu_ratio=statistics.median(ratios),ratio_range=[min(ratios),max(ratios)],expected=str(expected),expected_sha256=sha(expected)))
for label in coordinates:assert len(coordinates[label])>0
O=D/'dedup-gradient-masked-lanes-managed-benchmark-audit-20261014.json';assert not O.exists()
import shutil
retained=D/'dedup-gradient-masked-lanes-managed-benchmark-20261014';retained.mkdir(exist_ok=False)
for artifact in folder.iterdir():
    if artifact.is_file() and artifact.suffix in ['.rs','.txt','.stdout','.stderr']:
        copied=retained/artifact.name;shutil.copyfile(artifact,copied);assert sha(copied)==sha(artifact)
O.write_text(json.dumps(dict(status='verified_public_managed_matcher_version_comparison',rows=records,timed_fresh_processes=80,warmup_pairs_per_group=1,measured_pairs_per_group=9,pins={str(p):sha(p) for p in [P,Path(__file__)]},scope=d['scope']+' Output coordinate bits and allocator peaks match across versions and both memory paths. Selective second-pass reverse-lane branch is the only source difference; shared kernel unchanged. This is component CPU including startup/input-load/output/teardown; not whole native search or universal performance evidence.'),indent=2)+'\n');print(records)
