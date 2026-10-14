#!/usr/bin/env python3
from pathlib import Path
import subprocess,json,hashlib,os,re,statistics,time,resource
R=Path(__file__).resolve().parents[1];D=R/'docs/research';A=D/'dedup-collection-fused-preverify-v3-opt3-audit-20261015.json';a=json.loads(A.read_text());assert a['passed']==56 and a['failed']==0
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();exe=Path(a['executable']);assert sha(exe)==a['executable_sha256'];F=Path('/private/tmp/rrrah-collection-fused-preverify-v3-opt3-benchmark-20261015');F.mkdir(exist_ok=False);O=D/'dedup-collection-fused-preverify-v3-opt3-benchmark-20261015.json';assert not O.exists();cases=[('empty2',2,0,1000,'equal'),('empty8',8,0,200,'equal'),('64k8',8,65536,50,'equal'),('64k24',24,65536,5,'equal'),('2m8',8,2*1024*1024,3,'equal'),('early8',8,65536,50,'early'),('late8',8,65536,50,'late'),('sizes8',8,65536,50,'sizes'),('mixed8',8,65536,50,'mixed')];d=dict(status='running',producer_pid=os.getpid(),producer_sha256=sha(__file__),correctness_audit=str(A),correctness_audit_sha256=sha(A),executable=str(exe),executable_sha256=sha(exe),rows=[],scope='Alternating AB/BA freshprocess samebinary opt3synthetic guardedpair loop, original5cases plus4negative/mixed cases, warm OS cache expected. Includes capture/initial/prepair/bytecompare/postpair/postloop/callbackfree final validation. ChildCPUdelta includes whole freshprocess setup/harness/tempcleanup, while Rustwalltimer excludes initial filecreation. No actual collection/provider/decode, Cargo release/LTO, cold deviceIO, hardRSS, Windows or wholelibrary speedup proof.')
def save():
 t=O.with_suffix('.tmp');t.write_text(json.dumps(d,indent=2)+'\n');t.replace(O)
save()
for label,n,size,iterations,mode in cases:
 for round in range(7):
  for variant in (['baseline','candidate']if round%2==0 else['candidate','baseline']):
   assert sha(exe)==d['executable_sha256'];env=os.environ.copy();env.update(DEDUP_BENCH_FILES=str(n),DEDUP_BENCH_BYTES=str(size),DEDUP_BENCH_ITERATIONS=str(iterations),DEDUP_BENCH_VARIANT=variant,DEDUP_BENCH_CONTENT=mode);cmd=[str(exe),'--ignored','--exact','exact::fused_preverify_benchmark::full_exact_guard_loop','--nocapture','--test-threads','1'];L=F/f'{label}-{round}-{variant}.log'
   cpu_before=resource.getrusage(resource.RUSAGE_CHILDREN)
   with L.open('x')as log:
    child=subprocess.Popen(cmd,cwd=R,env=env,stdout=log,stderr=subprocess.STDOUT);d.update(active=dict(case=label,round=round,variant=variant,child_pid=child.pid));save();rc=child.wait()
   cpu_after=resource.getrusage(resource.RUSAGE_CHILDREN);cpu_seconds=(cpu_after.ru_utime-cpu_before.ru_utime)+(cpu_after.ru_stime-cpu_before.ru_stime)
   text=L.read_text();m=re.search(r'DEDUP_BENCH_NS=(\d+) ACCEPTED=(\d+)',text);assert rc==0 and m and int(m[2])==iterations*n*(n-1)//2 and '1 passed; 0 failed'in text;matched=re.search(r'MATCHED=(\d+)',text);expected_matches=iterations*(n*(n-1)//2 if mode=='equal'else sum(1 for i in range(n)for j in range(i+1,n)if mode=='mixed'and i%3==j%3));assert matched and int(matched[1])==expected_matches;d['rows'].append(dict(content_mode=mode,matched=expected_matches,cpu_seconds=cpu_seconds,case=label,files=n,bytes=size,iterations=iterations,round=round,variant=variant,nanoseconds=int(m[1]),accepted=int(m[2]),log=str(L),log_sha256=sha(L)));save()
results=[]
for label,*_ in cases:
 ratios=[];cpu_ratios=[]
 for i in range(7):
  rows={r['variant']:r for r in d['rows']if r['case']==label and r['round']==i};ratios.append(rows['candidate']['nanoseconds']/rows['baseline']['nanoseconds'])
  cpu_ratios.append(rows['candidate']['cpu_seconds']/rows['baseline']['cpu_seconds'])
 results.append(dict(case=label,paired_ratios=ratios,median_ratio=statistics.median(ratios),paired_cpu_ratios=cpu_ratios,median_cpu_ratio=statistics.median(cpu_ratios)))
assert sha(exe)==d['executable_sha256'] and sha(A)==d['correctness_audit_sha256'];d.update(status='completed_pending_independent_audit',results=results,active=None);save();print(results)
