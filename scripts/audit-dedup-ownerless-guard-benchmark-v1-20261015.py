#!/usr/bin/env python3
from pathlib import Path
import json,hashlib,re,statistics
R=Path(__file__).resolve().parents[1];D=R/'docs/research';P=D/'dedup-ownerless-guard-v1-benchmark-20261015.json';d=json.loads(P.read_text());sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();assert d['status']=='completed_pending_independent_audit'and len(d['rows'])==168 and d['active']is None;assert sha(d['executable'])==d['executable_sha256']and sha(d['audit'])==d['audit_sha256']and sha(R/'scripts/benchmark-dedup-ownerless-guard-v1-20261015.py')==d['producer_sha256'];cursor=0;results=[]
for n in [2,8,64,256]:
 for kind in ['none','all','mixed']:
  ratios=[];cpus=[]
  for turn in range(7):
   pair={}
   for variant in (['baseline','candidate']if turn%2==0 else['candidate','baseline']):
    row=d['rows'][cursor];cursor+=1;assert(row['files'],row['kind'],row['round'],row['variant'])==(n,kind,turn,variant);assert sha(row['log'])==row['log_sha256'];text=Path(row['log']).read_text();m=re.search(r'OWNER_BENCH_NS=(\d+) CALLS=(\d+)',text);assert m and int(m[1])==row['nanoseconds']>0 and int(m[2])==row['callback_calls']==100000 and row['child_cpu_seconds']>0 and '1 passed; 0 failed'in text;pair[variant]=row
   ratios.append(pair['candidate']['nanoseconds']/pair['baseline']['nanoseconds']);cpus.append(pair['candidate']['child_cpu_seconds']/pair['baseline']['child_cpu_seconds'])
  result=dict(files=n,kind=kind,paired_wall_ratios=ratios,median_wall_ratio=statistics.median(ratios),paired_cpu_ratios=cpus,median_cpu_ratio=statistics.median(cpus));assert result==next(r for r in d['results']if r['files']==n and r['kind']==kind);results.append(result)
O=D/'dedup-ownerless-guard-v1-benchmark-audit-20261015.json';assert not O.exists();O.write_text(json.dumps(dict(status='verified168_component_processes_print_only_producer_failure',producer_report=str(P),producer_report_sha256=sha(P),auditor_sha256=sha(__file__),fresh_processes=168,results=results,producer_terminal_scope='Producer exited1 after completing all168rows, asserting finalbinary/audit pins, computing and atomicallysaving completedreport. Lastprint raised TypeError because loopvariable round shadowed builtinround. Original buggyproducer preserved; this independentauditor verifies savedlogs/counts/ratios. No benchmarkprocess failure observed, no restart or alteredrawmeasurements.',scope=d['scope']),indent=2)+'\n');print([(r['files'],r['kind'],format(r['median_wall_ratio'],'.4f'),format(r['median_cpu_ratio'],'.4f'))for r in results])
