#!/usr/bin/env python3
from pathlib import Path
import hashlib,json,statistics,struct,shutil
R=Path(__file__).resolve().parents[1];D=R/'docs/research';P=D/'dedup-gradient-lanes16-public-20261014.json';d=json.loads(P.read_text());sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert d['status']=='complete_pending_audit'and len(d['cases'])==7 and len(d['results'])==12
assert all(sha(p)==h for p,h in d['pins'].items())and all(sha(p)==h for p,h in d['dependency_pins'].items())and all(sha(p)==h for p,h in d['input_pins'].items())
assert len(d['builds'])==4
for row in d['builds']:assert row['returncode']==0 and sha(row['log'])==row['log_sha256']
F=Path('/private/tmp/rrrah-gradient-lanes16-public-20261014')
for version in ['baseline','candidate']:assert (F/version/'describe.stdout').read_text().strip()=='1040 aarch64 macos'
counts={}
for p in d['input_pins']:
    data=Path(p).read_bytes();n=struct.unpack('<Q',data[:8])[0];assert len(data)==8+n*1040;counts[p]=n
summary=[];coordinates={};raw_files=[];total_calls=0
for case in d['cases']:
    for extra in case['extras']:
        group=[r for r in d['results']if r['label']==case['label']and r['extra_bytes']==extra];assert len(group)==1;r=group[0];assert r['inputs']==case['inputs']and r['repetitions']==case['repetitions']
        left,right=map(counts.__getitem__,case['inputs']);base=(left+right)*24+min(left,right)*32;peak=base
        if extra and 4096<=left*right<=16*1024*1024//8:peak+=left*right*8
        elif extra and left*right>16*1024*1024//8:peak+=(left+right)*16
        assert len(r['rows'])==20;canonical=None
        for i,row in enumerate(r['rows']):
            pair=i//2;versions=['baseline','candidate']if pair%2==0 else ['candidate','baseline'];version=versions[i%2]
            assert row['pair']==pair and row['version']==version and row['returncode']==0 and row['cpu_seconds']>0 and row['wall_seconds']>0
            assert row['command']==[str(F/version/'probe'),*case['inputs'],str(extra),str(case['repetitions'])]
            assert sha(row['stdout'])==row['stdout_sha256']and sha(row['stderr'])==row['stderr_sha256']and Path(row['stderr']).read_bytes()==b''
            blob=Path(row['stdout']).read_bytes()
            if canonical is None:canonical=blob
            assert blob==canonical;raw_files.extend([row['stdout'],row['stderr']]);total_calls+=case['repetitions']
        lines=canonical.decode().splitlines();matched,actual_peak=map(int,lines[0].split());assert matched==r['matches']and actual_peak==r['peak_bytes']==peak and len(lines)==matched+1
        values=[list(map(int,l.split()))for l in lines[1:]];assert all(len(v)==4 and all(0<=x<2**64 for x in v)for v in values)
        if case['label']in coordinates:assert coordinates[case['label']]==values
        else:coordinates[case['label']]=values
        ratios=[]
        for pair in range(1,10):
            cpu={x['version']:x['cpu_seconds']for x in r['rows']if x['pair']==pair};ratios.append(cpu['candidate']/cpu['baseline'])
        assert ratios==r['ratios']and statistics.median(ratios)==r['median_cpu_ratio']
        summary.append(dict(label=case['label'],extra_bytes=extra,repetitions_per_process=case['repetitions'],matches=matched,peak_bytes=peak,median_cpu_ratio=statistics.median(ratios),ratio_range=[min(ratios),max(ratios)]))
assert total_calls==2100200
retained=D/'dedup-gradient-lanes16-public-20261014';retained.mkdir(exist_ok=False);pins={}
for name in raw_files+[str(F/'probe.rs')]+[str(F/v/'describe.stdout')for v in ['baseline','candidate']]:
    p=Path(name);label=p.name if p.name!='describe.stdout'else p.parent.name+'-describe.stdout';copy=retained/label;shutil.copyfile(p,copy);assert sha(p)==sha(copy);pins[str(copy.relative_to(R))]=sha(copy)
O=D/'dedup-gradient-lanes16-public-audit-20261014.json';assert not O.exists();O.write_text(json.dumps(dict(status='verified_actual_layout_public_managed_component_gain',report=str(P.relative_to(R)),report_sha256=sha(P),auditor_sha256=sha(__file__),feature_stride_bytes=1040,observed_target='aarch64-macos',fresh_processes=240,actual_matcher_calls=total_calls,measured_pairs=108,warmup_pairs_per_group=1,summary=summary,retained_raw_outputs=pins,exact_coordinates_and_allocator_peaks_across_versions=True,exact_coordinates_across_headroom_paths=True,scope=d['scope'],adoption_limit='Observed MacARMcomponent only; no universal target or full native operation/corpus/default/UI/decoder proof. Firstpass16must keep secondpass4skip behavior. NonMac performance not measured.'),indent=2)+'\n');print(summary)
