"""Independently audit all126 actual-engine paired rows, source/gate/profile/output pins."""
from pathlib import Path
import hashlib,json,re,runpy,statistics
R=Path(__file__).resolve().parents[1];D=R/'docs/research';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 report=D/'dedup-durable-actual-engine-v10-benchmark-20261016.json';d=json.loads(report.read_text());assert d['status']=='completed_pending_independent_audit' and len(d['rows'])==126;assert h(Path(d['binary']))==d['binary_sha256'];assert h(R/'scripts/benchmark-dedup-durable-actual-engine-v10-20261016.py')==d['producer_sha256'];protocol=Path(d['protocol']);assert h(protocol)==d['protocol_sha256'];p=json.loads(protocol.read_text());build=Path(d['build_report']);assert h(build)==d['build_report_sha256'];b=json.loads(build.read_text());assert b['binary_sha256']==d['binary_sha256'] and b['protocol_sha256']==h(protocol) and h(R/'scripts/build-dedup-durable-actual-benchmark-v5-20261016.py')==b['producer_sha256'];assert h(R/p['gate_admission'])==p['gate_admission_sha256'];runpy.run_path(str(R/p['gate_admission']))
 prior=Path(d['previous_unqualified_attempt_audit']);assert h(prior)==d['previous_unqualified_attempt_audit_sha256'];a=json.loads(prior.read_text());assert a['status']=='verified_terminal_attempt_unqualified_due_observed_compiler' and h(R/a['report'])==a['report_sha256']
 for key,value in p['compiler_profile_required'].items():assert b['profile'][key]==value
 for path,sha in b['logs'].items():assert h(path)==sha
 assert h(b['inventory'])==b['inventory_sha256'] and p['test_name']+': test' in Path(b['inventory']).read_text()
 for variant,sha in b['full_gate_pins'].items():assert h(D/f'dedup-durable-{variant}-audit-v5-20261015.json')==sha
 assert p['rounds']==7 and len(p['cases'])==9;cursor=0;results=[]
 for case in p['cases']:
  n=case['files'];pairs=n*(n-1)//2;equal=pairs if case['content']=='equal' else sum(i%3==j%3 for i in range(n) for j in range(i+1,n)) if case['content']=='mixed' else 0;visual=case['iterations']*(pairs-equal);ratios=[];cpus=[]
  for round_id in range(7):
   pair={}
   for variant in (['baseline','candidate'] if round_id%2==0 else ['candidate','baseline']):
    row=d['rows'][cursor];cursor+=1;assert (row['case'],row['files'],row['bytes'],row['iterations'],row['content'],row['round'],row['variant'])==(case['label'],n,case['bytes'],case['iterations'],case['content'],round_id,variant);assert h(row['log'])==row['log_sha256'];text=Path(row['log']).read_text();match=re.search(r'DEDUP_ENGINE_NS=(\d+) ACCEPTED=(\d+) VISUAL_CALLS=(\d+) CHECKPOINTS=(\d+)',text);assert match and '1 passed; 0 failed;' in text;assert list(map(int,match.groups()))==[row['nanoseconds'],row['accepted'],row['visual_calls'],row['checkpoints']];assert row['accepted']==case['iterations']*pairs and row['visual_calls']==visual and row['nanoseconds']>0 and row['child_cpu_seconds']>0;assert not row['observed_build_processes_before'] and not row['observed_build_processes_after'];pair[variant]=row
   ratios.append(pair['candidate']['nanoseconds']/pair['baseline']['nanoseconds']);cpus.append(pair['candidate']['child_cpu_seconds']/pair['baseline']['child_cpu_seconds'])
  results.append({'case':case['label'],'paired_ratios':ratios,'median_ratio':statistics.median(ratios),'paired_cpu_ratios':cpus,'median_cpu_ratio':statistics.median(cpus)})
 assert results==d['results'] and cursor==126;out=D/'dedup-durable-actual-engine-v10-benchmark-audit-20261016.json';assert not out.exists();out.write_text(json.dumps({'status':'verified126paired_actual_engine_process_results','report':str(report),'report_sha256':h(report),'auditor_sha256':h(__file__),'results':results,'scope':d['scope']+' Independentoutputcount/visualcalls/order/source/profile/rawhash/ratio checks. No statisticalsignificance/librarywidespeedup/adoption claim.'},indent=2)+'\n');print(json.dumps(results))
if __name__=='__main__':main()
