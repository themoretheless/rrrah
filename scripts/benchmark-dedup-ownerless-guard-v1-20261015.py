#!/usr/bin/env python3
from pathlib import Path
import json,hashlib,subprocess,os,re,statistics,resource
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();A=D/'dedup-ownerless-guard-v1-audit-20261015.json';a=json.loads(A.read_text());assert a['passed']==2;exe=a['executable'];assert sha(exe)==a['executable_sha256'];F=Path('/private/tmp/rrrah-ownerless-guard-v1-benchmark-20261015');F.mkdir(exist_ok=False);O=D/'dedup-ownerless-guard-v1-benchmark-20261015.json';assert not O.exists();d=dict(status='running',producer_pid=os.getpid(),producer_sha256=sha(__file__),audit=str(A),audit_sha256=sha(A),executable=exe,executable_sha256=sha(exe),rows=[],scope='168freshprocess samebinary opt3component runs,4requestcounts2/8/64/256 x3ownerkindsnone/all/mixed x7pairedABBArounds.100000checkpoints each; actual immutable requests/tokens, opaque dynFn callback, ownerpresence selection included once in Rustwalltimer. No fullcollection/provider/decode/wholelibrary speedup, coldIO, RSS or platform qualification. Concurrent fullintegration compilation and nativecampaign may affect timings; retain alltoken cases and raw variation. ChildCPUdelta includes wholeprocessstartup, Rustwall excludes requestsetup.')
def save():
 t=O.with_suffix('.tmp');t.write_text(json.dumps(d,indent=2)+'\n');t.replace(O)
save()
for n in [2,8,64,256]:
 for kind in ['none','all','mixed']:
  for round in range(7):
   for variant in (['baseline','candidate']if round%2==0 else['candidate','baseline']):
    assert sha(exe)==d['executable_sha256'];env=os.environ.copy();env.update(OWNER_BENCH_FILES=str(n),OWNER_BENCH_KIND=kind,OWNER_BENCH_VARIANT=variant);L=F/f'{n}-{kind}-{round}-{variant}.log';before=resource.getrusage(resource.RUSAGE_CHILDREN)
    with L.open('x')as log:
     child=subprocess.Popen([exe,'--ignored','--exact','checkpoint_loop','--nocapture','--test-threads','1'],cwd=R,env=env,stdout=log,stderr=subprocess.STDOUT);d.update(active=dict(files=n,kind=kind,round=round,variant=variant,child_pid=child.pid));save();rc=child.wait()
    after=resource.getrusage(resource.RUSAGE_CHILDREN);cpu=(after.ru_utime-before.ru_utime)+(after.ru_stime-before.ru_stime);text=L.read_text();m=re.search(r'OWNER_BENCH_NS=(\d+) CALLS=(\d+)',text);assert rc==0 and m and int(m[2])==100000 and '1 passed; 0 failed'in text and cpu>0;d['rows'].append(dict(files=n,kind=kind,round=round,variant=variant,nanoseconds=int(m[1]),callback_calls=int(m[2]),child_cpu_seconds=cpu,log=str(L),log_sha256=sha(L)));save()
results=[]
for n in [2,8,64,256]:
 for kind in ['none','all','mixed']:
  ratios=[];cpus=[]
  for round in range(7):
   rows={r['variant']:r for r in d['rows']if r['files']==n and r['kind']==kind and r['round']==round};ratios.append(rows['candidate']['nanoseconds']/rows['baseline']['nanoseconds']);cpus.append(rows['candidate']['child_cpu_seconds']/rows['baseline']['child_cpu_seconds'])
  results.append(dict(files=n,kind=kind,paired_wall_ratios=ratios,median_wall_ratio=statistics.median(ratios),paired_cpu_ratios=cpus,median_cpu_ratio=statistics.median(cpus)))
assert sha(exe)==d['executable_sha256']and sha(A)==d['audit_sha256'];d.update(status='completed_pending_independent_audit',active=None,results=results);save();print([(r['files'],r['kind'],round(r['median_wall_ratio'],4))for r in results])
