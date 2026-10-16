"""Admission-gated samebinary actualengine timing; never adopts code or stops other jobs."""
from pathlib import Path
import argparse,hashlib,json,os,re,resource,runpy,statistics,subprocess,time
R=Path(__file__).resolve().parents[1];D=R/'docs/research';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
def process_snapshot():
 result=subprocess.run(['ps','-axo','pid=,ppid=,comm='],capture_output=True,text=True,check=True);rows=[]
 for line in result.stdout.splitlines():
  parts=line.split(None,2)
  if len(parts)!=3:continue
  pid,parent,command=parts;name=Path(command).name
  if name in {'cargo','rustc','clang','clang++','cc','gcc','g++','cc1','cc1plus','ld','ld.lld','lld','ninja','make','cmake','swiftc','swift-frontend','dotnet'}:rows.append({'pid':int(pid),'parent_pid':int(parent),'executable':command})
 return rows
def main():
 prior=D/'dedup-durable-actual-engine-v13-interference-audit-20261016.json';a=json.loads(prior.read_text());assert a['status']=='verified_terminal_attempt_unqualified_due_observed_compiler' and h(R/a['report'])==a['report_sha256']
 parser=argparse.ArgumentParser();parser.add_argument('--check-only',action='store_true');args=parser.parse_args();P=D/'dedup-durable-actual-benchmark-v5-protocol-20261016.json';p=json.loads(P.read_text());build=D/'dedup-durable-actual-benchmark-v5-build-20261016.json'
 if not build.exists():
  assert args.check_only,'Opt3 binary not built/admitted; no timing'
  print(json.dumps({'status':'waiting_for_full_correctness_and_opt3_build','timed':False}));return
 b=json.loads(build.read_text());assert h(P)==b['protocol_sha256'] and b['status']=='compiled_opt3_not_timed';assert h(R/p['gate_admission'])==p['gate_admission_sha256'];runpy.run_path(str(R/p['gate_admission']));assert h(R/'scripts/build-dedup-durable-actual-benchmark-v5-20261016.py')==b['producer_sha256'];exe=Path(b['binary']);assert h(exe)==b['binary_sha256'];heavy=process_snapshot()
 if args.check_only:print(json.dumps({'status':'timing_admitted' if not heavy else 'waiting_for_observed_build_processes','observed_build_processes':heavy,'timed':False}));return
 assert not heavy,'Active observedbuildprocesses; do not stop them orlaunch timing';time.sleep(1);assert not process_snapshot(),'Observedbuild activity during preflight'
 W=Path(p['work'])/'timing-v14';W.mkdir(exist_ok=False);out=D/'dedup-durable-actual-engine-v14-benchmark-20261016.json';assert not out.exists();d={'status':'running','producer_pid':os.getpid(),'previous_unqualified_attempt_audit':str(prior),'previous_unqualified_attempt_audit_sha256':h(prior),'producer_sha256':h(__file__),'protocol':str(P),'protocol_sha256':h(P),'build_report':str(build),'build_report_sha256':h(build),'binary':str(exe),'binary_sha256':h(exe),'rows':[],'active':None,'scope':'Actualscan_guarded_exact_collection completeengine, ninefamilies sevenAB/BApairedrounds, sameopt3binary; exactbyteoracle syntheticprovider notactualimagevisualalgorithm. Rustwalltimer excludesinitialfilecreation; childCPU includesfreshprocess/setup/cleanup. No observedcompiler/buildprocesses before/aftereachrow; otherhostwork canstillaffecttiming. Warmcache expected, notcoldphysicalIO/hardRSS/platform/fullmutableRoot/productionadoption proof.'}
 def save():
  tmp=out.with_suffix('.tmp');tmp.write_text(json.dumps(d,indent=2)+'\n');tmp.replace(out)
 save()
 try:
  for case in p['cases']:
   n=case['files'];pairs=n*(n-1)//2;mode=case['content'];equal_pairs=pairs if mode=='equal' else sum(i%3==j%3 for i in range(n) for j in range(i+1,n)) if mode=='mixed' else 0;expected_visual=case['iterations']*(pairs-equal_pairs)
   for round_id in range(p['rounds']):
    for variant in (['baseline','candidate'] if round_id%2==0 else ['candidate','baseline']):
     before=process_snapshot();d['last_build_process_observation']=before;assert not before,'Observedcompiler/buildprocess before sample';assert h(exe)==b['binary_sha256'];env=dict(os.environ,DEDUP_BENCH_FILES=str(n),DEDUP_BENCH_BYTES=str(case['bytes']),DEDUP_BENCH_ITERATIONS=str(case['iterations']),DEDUP_BENCH_VARIANT=variant,DEDUP_BENCH_CONTENT=mode);cmd=[str(exe),'--ignored','--exact',p['test_name'],'--nocapture','--test-threads','1'];log=W/f"{case['label']}-{round_id}-{variant}.log";cpu0=resource.getrusage(resource.RUSAGE_CHILDREN)
     with log.open('x') as stream:
      child=subprocess.Popen(cmd,cwd=p['source'],env=env,stdout=stream,stderr=subprocess.STDOUT);d['active']={'case':case['label'],'round':round_id,'variant':variant,'child_pid':child.pid};save();code=child.wait(timeout=120)
     cpu1=resource.getrusage(resource.RUSAGE_CHILDREN);seconds=(cpu1.ru_utime-cpu0.ru_utime)+(cpu1.ru_stime-cpu0.ru_stime);after=process_snapshot();d['last_build_process_observation']=after;body=log.read_text();match=re.search(r'DEDUP_ENGINE_NS=(\d+) ACCEPTED=(\d+) VISUAL_CALLS=(\d+) CHECKPOINTS=(\d+)',body);assert code==0 and match and '1 passed; 0 failed;' in body and int(match[2])==case['iterations']*pairs and int(match[3])==expected_visual and int(match[1])>0 and seconds>0;assert not after,'Observedcompiler/buildprocess aftersample; timingunqualified'
     d['rows'].append({'case':case['label'],'files':n,'bytes':case['bytes'],'iterations':case['iterations'],'content':mode,'round':round_id,'variant':variant,'nanoseconds':int(match[1]),'accepted':int(match[2]),'visual_calls':int(match[3]),'checkpoints':int(match[4]),'child_cpu_seconds':seconds,'observed_build_processes_before':before,'observed_build_processes_after':after,'log':str(log),'log_sha256':h(log)});save()
  results=[]
  for case in p['cases']:
   ratios=[];cpu_ratios=[]
   for round_id in range(p['rounds']):
    rows={r['variant']:r for r in d['rows'] if r['case']==case['label'] and r['round']==round_id};ratios.append(rows['candidate']['nanoseconds']/rows['baseline']['nanoseconds']);cpu_ratios.append(rows['candidate']['child_cpu_seconds']/rows['baseline']['child_cpu_seconds'])
   results.append({'case':case['label'],'paired_ratios':ratios,'median_ratio':statistics.median(ratios),'paired_cpu_ratios':cpu_ratios,'median_cpu_ratio':statistics.median(cpu_ratios)})
  assert len(d['rows'])==p['fresh_processes']==126 and h(exe)==b['binary_sha256'];d.update(status='completed_pending_independent_audit',results=results,active=None);save();print(json.dumps(results))
 except BaseException as error:
  if 'child' in locals() and child.poll() is None:
   child.terminate()
   try:child.wait(timeout=5)
   except subprocess.TimeoutExpired:child.kill();child.wait(timeout=5)
   d['owned_child_cleanup_returncode']=child.returncode
  d.update(status='stopped_unqualified',error_type=type(error).__name__,error=str(error));save();raise
if __name__=='__main__':main()
