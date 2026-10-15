"""Correctness smoke of all nine benchmark families; wall timings deliberately unqualified."""
from pathlib import Path
import hashlib,json,os,re,runpy,subprocess
R=Path(__file__).resolve().parents[1];D=R/'docs/research';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();P=D/'dedup-durable-actual-benchmark-v5-protocol-20261016.json';p=json.loads(P.read_text());B=D/'dedup-durable-actual-benchmark-v5-build-20261016.json';b=json.loads(B.read_text());assert b['protocol_sha256']==h(P);assert h(R/'scripts/build-dedup-durable-actual-benchmark-v5-20261016.py')==b['producer_sha256'];runpy.run_path(str(R/p['gate_admission']))
for name,digest in b['logs'].items():assert h(name)==digest
assert h(b['inventory'])==b['inventory_sha256']
for key,value in p['compiler_profile_required'].items():assert b['profile'][key]==value
W=Path(p['work'])/'correctness-controls';W.mkdir(exist_ok=False);rows=[];binary=Path(b['binary']);assert h(binary)==b['binary_sha256']
for repeat in range(2):
 for case in p['cases']:
  n=case['files'];pairs=n*(n-1)//2;equal=pairs if case['content']=='equal' else sum(i%3==j%3 for i in range(n) for j in range(i+1,n)) if case['content']=='mixed' else 0
  for variant in ['baseline','candidate']:
   env=dict(os.environ,DEDUP_BENCH_FILES=str(n),DEDUP_BENCH_BYTES=str(case['bytes']),DEDUP_BENCH_ITERATIONS='1',DEDUP_BENCH_VARIANT=variant,DEDUP_BENCH_CONTENT=case['content']);log=W/f"{repeat}-{case['label']}-{variant}.log";cmd=[str(binary),'--ignored','--exact',p['test_name'],'--nocapture','--test-threads','1']
   with log.open('x') as stream:result=subprocess.run(cmd,cwd=p['source'],env=env,stdout=stream,stderr=subprocess.STDOUT,timeout=60)
   text=log.read_text();m=re.search(r'DEDUP_ENGINE_NS=(\d+) ACCEPTED=(\d+) VISUAL_CALLS=(\d+) CHECKPOINTS=(\d+)',text);assert result.returncode==0 and m and '1 passed; 0 failed;' in text;assert int(m[2])==pairs and int(m[3])==pairs-equal
   rows.append({'repeat':repeat,'case':case['label'],'files':n,'bytes':case['bytes'],'content':case['content'],'iterations':1,'variant':variant,'accepted':int(m[2]),'visual_calls':int(m[3]),'checkpoints':int(m[4]),'log':str(log.relative_to(R)),'log_sha256':h(log)})
assert len(rows)==36 and h(binary)==b['binary_sha256'];out=D/'dedup-durable-opt3-engine-controls-v5-20261016.json';assert not out.exists();out.write_text(json.dumps({'status':'verified36_actual_opt3_engine_correctness_processes','producer_sha256':h(__file__),'build_report':str(B.relative_to(R)),'build_report_sha256':h(B),'binary_sha256':h(binary),'protocol_sha256':h(P),'rows':rows,'scope':'All9originalprotocol families, bothbaseline/candidate branches twice, onecompleteactual guardedcollection operation/process. Internal byteoracle pair evidence, provider call count and budgetrelease assertions executed; no code/data adapters. Concurrent host compiler activity: timings intentionally omitted/unqualified. Not completeopt3unit/integrationtest suite, actualimageprovider/decodequality, allplatform/fullmutableRoot/performance/adoption proof.'},indent=2)+'\n');print('All9families/bothbranches twice:36freshopt3engineprocesses passed; no timing claim')
