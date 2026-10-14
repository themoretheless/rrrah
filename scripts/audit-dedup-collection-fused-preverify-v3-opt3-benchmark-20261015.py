#!/usr/bin/env python3
from pathlib import Path
import hashlib,json,re,statistics,math
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
P=D/'dedup-collection-fused-preverify-v3-opt3-benchmark-20261015.json';d=json.loads(P.read_text());assert d['status']=='completed_pending_independent_audit';assert sha(d['executable'])==d['executable_sha256'];assert sha(d['correctness_audit'])==d['correctness_audit_sha256'];assert sha(R/'scripts/benchmark-dedup-collection-fused-preverify-v3-opt3-20261015.py')==d['producer_sha256'];assert len(d['rows'])==126
expected=[('empty2',2,0,1000,'equal'),('empty8',8,0,200,'equal'),('64k8',8,65536,50,'equal'),('64k24',24,65536,5,'equal'),('2m8',8,2*1024*1024,3,'equal'),('early8',8,65536,50,'early'),('late8',8,65536,50,'late'),('sizes8',8,65536,50,'sizes'),('mixed8',8,65536,50,'mixed')];results=[];cursor=0
for label,n,size,iterations,mode in expected:
 ratios=[];cpu_ratios=[]
 for round in range(7):
  times={};cpus={}
  for variant in (['baseline','candidate']if round%2==0 else['candidate','baseline']):
   row=d['rows'][cursor];cursor+=1;assert (row['case'],row['files'],row['bytes'],row['iterations'],row['round'],row['variant'])==(label,n,size,iterations,round,variant);assert sha(row['log'])==row['log_sha256'];text=Path(row['log']).read_text();m=re.search(r'DEDUP_BENCH_NS=(\d+) ACCEPTED=(\d+)',text);assert m and int(m[1])==row['nanoseconds']>0 and int(m[2])==row['accepted']==iterations*n*(n-1)//2 and '1 passed; 0 failed'in text;times[variant]=row['nanoseconds'];assert row['content_mode']==mode and row['cpu_seconds']>0;expected_matches=iterations*(n*(n-1)//2 if mode=='equal'else sum(1 for i in range(n)for j in range(i+1,n)if mode=='mixed'and i%3==j%3));matched=re.search(r'MATCHED=(\d+)',text);assert matched and int(matched[1])==row['matched']==expected_matches;cpus[variant]=row['cpu_seconds']
  ratios.append(times['candidate']/times['baseline']);cpu_ratios.append(cpus['candidate']/cpus['baseline'])
 result=dict(case=label,paired_ratios=ratios,median_ratio=statistics.median(ratios),paired_cpu_ratios=cpu_ratios,median_cpu_ratio=statistics.median(cpu_ratios));old=next(v for v in d['results']if v['case']==label);assert result==old;results.append(result)
O=D/'dedup-collection-fused-preverify-v3-opt3-benchmark-audit-20261015.json';assert not O.exists();O.write_text(json.dumps(dict(status='verified126process_opt3_synthetic_guard_loop_wall_and_childcpu',producer_report=str(P.relative_to(R)),producer_report_sha256=sha(P),auditor_sha256=sha(__file__),fresh_processes=126,paired_rounds_per_case=7,results=results,scope=d['scope']+' Independent roworder/input/output/count/hash/ratio checks. Concurrent compiler/campaign activity may affect walltiming; no statistical significance claim or production adoption from this diagnostic alone.'),indent=2)+'\n');print(results)
