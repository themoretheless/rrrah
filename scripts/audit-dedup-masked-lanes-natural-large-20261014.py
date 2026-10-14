#!/usr/bin/env python3
from pathlib import Path
import hashlib,json,statistics,struct,shutil
R=Path(__file__).resolve().parents[1];D=R/'docs/research';P=D/'dedup-masked-lanes-natural-large-20261014.json';d=json.loads(P.read_text());sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert d['status']=='complete_pending_audit'and len(d['cases'])==3 and len(d['results'])==6
assert all(sha(p)==h for p,h in d['pins'].items())and all(sha(p)==h for p,h in d['input_pins'].items())and all(sha(p)==h for p,h in d['dependency_pins'].items())
fixture_audit=D/'dedup-natural-large-gradient-banks-v2-audit-20261014.json';fixture=json.loads(fixture_audit.read_text());assert fixture['status']=='verified_two_large_natural_primitive_banks'
counts={p:struct.unpack('<Q',Path(p).read_bytes()[:8])[0]for p in d['input_pins']};assert all(n==2048 for n in counts.values())
summary=[];coordinates={};raw_files=[]
for case in d['cases']:
    for extra in [0,16*1024*1024]:
        matches=[r for r in d['results']if r['label']==case['label']and r['extra_bytes']==extra];assert len(matches)==1;result=matches[0]
        assert result['inputs']==case['inputs']and len(result['rows'])==20
        left,right=map(counts.__getitem__,case['inputs']);base=(left+right)*24+min(left,right)*32;expected_peak=base if extra==0 else base+(left+right)*16
        canonical=None
        for i,row in enumerate(result['rows']):
            pair=i//2;versions=['baseline','candidate']if pair%2==0 else ['candidate','baseline'];version=versions[i%2]
            assert row['pair']==pair and row['version']==version and row['returncode']==0 and row['cpu_seconds']>0 and row['wall_seconds']>0
            assert row['command']==[f'/private/tmp/rrrah-gradient-masked-lanes-managed-benchmark-v2-20261014/{version}/probe',*case['inputs'],str(extra)]
            assert sha(row['stdout'])==row['stdout_sha256']and sha(row['stderr'])==row['stderr_sha256']and Path(row['stderr']).read_bytes()==b''
            blob=Path(row['stdout']).read_bytes()
            if canonical is None:canonical=blob
            assert blob==canonical;raw_files.extend([row['stdout'],row['stderr']])
        lines=canonical.decode().splitlines();n,peak=map(int,lines[0].split());assert len(lines)==n+1 and n==result['matches']and peak==result['peak_bytes']==expected_peak
        points=[list(map(int,line.split()))for line in lines[1:]];assert all(len(p)==4 and all(0<=v<2**64 for v in p)for p in points)
        if case['label']in coordinates:assert points==coordinates[case['label']]
        else:coordinates[case['label']]=points
        ratios=[]
        for pair in range(1,10):
            cpu={r['version']:r['cpu_seconds']for r in result['rows']if r['pair']==pair};ratios.append(cpu['candidate']/cpu['baseline'])
        assert ratios==result['ratios']and statistics.median(ratios)==result['median_cpu_ratio']
        summary.append(dict(label=case['label'],extra_bytes=extra,matches=n,peak_bytes=peak,median_cpu_ratio=statistics.median(ratios),ratio_range=[min(ratios),max(ratios)]))
F=D/'dedup-masked-lanes-natural-large-20261014';F.mkdir(exist_ok=False);retained={}
for name in raw_files:
    p=Path(name);out=F/p.name;shutil.copyfile(p,out);assert sha(p)==sha(out);retained[str(out.relative_to(R))]=sha(out)
O=D/'dedup-masked-lanes-natural-large-audit-20261014.json';assert not O.exists();O.write_text(json.dumps(dict(status='verified_three_large_natural_public_managed_comparisons',report=str(P.relative_to(R)),report_sha256=sha(P),auditor_sha256=sha(__file__),fixture_audit=str(fixture_audit.relative_to(R)),fixture_audit_sha256=sha(fixture_audit),fresh_processes=120,measured_pairs=54,summary=summary,retained_raw_outputs=retained,exact_coordinates_and_allocator_peaks_across_versions=True,exact_coordinates_across_headroom_paths=True,scope=d['scope']),indent=2)+'\n');print(summary)
