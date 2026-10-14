#!/usr/bin/env python3
"""Reuse qualified public managed binaries on genuine larger natural banks."""
from pathlib import Path
import hashlib,json,os,subprocess,resource,time,statistics,struct
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
prerequisite=D/'dedup-gradient-masked-lanes-managed-benchmark-v2-20261014.json';prior=json.loads(prerequisite.read_text());assert prior['status']=='complete_pending_audit'
assert all(sha(p)==h for p,h in prior['pins'].items()) and all(sha(p)==h for p,h in prior['dependency_pins'].items())
fixture_path=D/'dedup-natural-large-gradient-banks-v2-20261014.json';fixtures=json.loads(fixture_path.read_text());assert fixtures['status']=='complete_pending_audit'
assert all(sha(p)==h for p,h in fixtures['pins'].items()) and all(sha(p)==h for p,h in fixtures['input_pins'].items())
features=[]
for f in fixtures['extractions']:
    assert f['returncode']==0 and sha(f['features'])==f['features_sha256']
    data=Path(f['features']).read_bytes();n=struct.unpack('<Q',data[:8])[0];assert 1536<n<=2048 and len(data)==8+1040*n
    features.append(f['features'])
assert len(features)==2
cases=[dict(label='natural_table_forward',inputs=features),dict(label='natural_table_reverse',inputs=features[::-1]),dict(label='natural_table_self',inputs=[features[0],features[0]])]
F=Path('/private/tmp/rrrah-masked-lanes-natural-large-20261014');F.mkdir(exist_ok=False)
O=D/'dedup-masked-lanes-natural-large-20261014.json';assert not O.exists()
state=dict(status='timing',producer_pid=os.getpid(),cases=cases,results=[],pins={str(p):sha(p)for p in [Path(__file__),prerequisite,fixture_path]},dependency_pins=prior['dependency_pins'],input_pins={p:sha(p)for p in features},scope='Actual public managed V7baseline versus isolated selective-lane candidate, reuse exact pinned optimized binaries and dependencies. Two genuinely extracted large natural banks, no descriptor repetition/padding; forward/reverse/self with baseline-only or16MiBextra linear-runner headroom. Ten alternating fresh-process pairs/group, first warmup excluded. Exact coordinate bits/allocator peaks and zero-credit assertion in original probe; component CPU includes startup/load/output. Pillow explicit sRGBfixture prep, not native decoder/ICC/fulloperation/default/corpus/platform or universal speedup proof.')
probes={v:Path('/private/tmp/rrrah-gradient-masked-lanes-managed-benchmark-v2-20261014')/v/'probe'for v in ['baseline','candidate']}
for p in probes.values():assert sha(p)==prior['pins'][str(p)];state['pins'][str(p)]=sha(p)
def save():
    tmp=O.with_suffix('.tmp');tmp.write_text(json.dumps(state,indent=2)+'\n');tmp.replace(O)
save()
for case in cases:
    for extra in [0,16*1024*1024]:
        rows=[];canonical=None
        for pair in range(10):
            for version in (['baseline','candidate']if pair%2==0 else ['candidate','baseline']):
                out=F/f"{case['label']}-{extra}-{pair}-{version}.stdout";err=out.with_suffix('.stderr');cmd=[str(probes[version]),*case['inputs'],str(extra)]
                with out.open('x')as stdout,err.open('x')as stderr:
                    before=resource.getrusage(resource.RUSAGE_CHILDREN);start=time.perf_counter()
                    p=subprocess.Popen(cmd,stdout=stdout,stderr=stderr);state.update(active_case=case['label'],active_extra=extra,active_pair=pair,active_version=version,child_pid=p.pid);save();code=p.wait()
                    elapsed=time.perf_counter()-start;after=resource.getrusage(resource.RUSAGE_CHILDREN)
                if canonical is None:canonical=out.read_bytes()
                assert code==0 and out.read_bytes()==canonical and err.read_bytes()==b'',(case,version,err.read_text())
                rows.append(dict(pair=pair,version=version,command=cmd,returncode=code,cpu_seconds=after.ru_utime+after.ru_stime-before.ru_utime-before.ru_stime,wall_seconds=elapsed,stdout=str(out),stdout_sha256=sha(out),stderr=str(err),stderr_sha256=sha(err)))
                state['active_rows']=rows;save()
        ratios=[]
        for pair in range(1,10):
            cpu={r['version']:r['cpu_seconds']for r in rows if r['pair']==pair};assert cpu['baseline']>0;ratios.append(cpu['candidate']/cpu['baseline'])
        matched,peak=map(int,canonical.splitlines()[0].split());state['results'].append(dict(**case,extra_bytes=extra,matches=matched,peak_bytes=peak,rows=rows,ratios=ratios,median_cpu_ratio=statistics.median(ratios)));save()
assert all(sha(p)==h for p,h in state['pins'].items()) and all(sha(p)==h for p,h in state['input_pins'].items())
state.pop('active_rows',None);state['status']='complete_pending_audit';save();print([(r['label'],r['extra_bytes'],r['median_cpu_ratio'])for r in state['results']])
