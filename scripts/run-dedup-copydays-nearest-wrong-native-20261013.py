#!/usr/bin/env python3
"""Precommit all229 SSCD-nearest wrong-origin pairs, then both native directions."""
import json,hashlib,os,subprocess,time,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
N=D/'dedup-sscd-copydays-nearest-wrong-20261013.json';A=D/'dedup-sscd-oracle-v2-audit-20261013.json';a=json.loads(A.read_text());assert sha(N)==a['nearest_wrong_sha256'];nearest=json.loads(N.read_text());assert nearest['pairs']==229
B=D/'dedup-current-native-bank-all-positives-20261012.json';base=json.loads(B.read_text());Q=D/'dedup-current-native-bank-complete-audit-20261013.json';q=json.loads(Q.read_text());assert q['completed']==458 and q['all_supported'] and all(sha(p)==h for p,h in q['pins'].items());binary=Path(base['executable']);weights=T/'rrrah-dedup-copydays/learned-matching-models/loftr-outdoor-native-20261010.safetensors';assert sha(binary)==base['pins'][str(binary)] and sha(weights)==base['pins'][str(weights)];S=Path(base['source']);manifest=json.loads(Path(base['source_manifest']).read_text());assert all(sha(S/p)==h for p,h in manifest['source_hashes'].items())
rows=sorted(nearest['rows'],key=lambda r:(r['positive_margin'],r['query'],r['reference']));assert len({r['query'] for r in rows})==229
plan=[]
for rank,r in enumerate(rows):
 assert Path(r['query']).stem[:4]==Path(r['origin']).stem[:4] and Path(r['query']).stem[:4]!=Path(r['reference']).stem[:4]
 for left,right,direction in [(r['query'],r['reference'],'query_to_wrong'),(r['reference'],r['query'],'wrong_to_query')]:plan.append(dict(left=left,right=right,direction=direction,origin=r['origin'],query=r['query'],wrong_reference=r['reference'],priority_rank=rank,publisher_retrieval_groundtruth='excluded_wrong_origin',sscd_positive_cosine=r['positive_cosine'],sscd_wrong_cosine=r['wrong_cosine'],sscd_positive_rank=r['positive_rank']))
pins={str(p):sha(p) for p in [N,A,B,Q,binary,weights,Path(base['source_manifest']),Path(__file__)]}
rt=D/'dedup-loftr-native-20261010-runtime-pins.json';pins[str(rt)]=sha(rt)
for p,h in json.loads(rt.read_text())['pins'].items():
 if p.endswith('.dylib'):assert sha(p)==h;pins[p]=h
for r in plan:
 for k in ['left','right','origin']:pins[r[k]]=sha(r[k])
P=D/'dedup-copydays-nearest-wrong-native-20261013.json';pre=D/'dedup-copydays-nearest-wrong-native-plan-20261013.json';F=T/'rrrah-copydays-nearest-wrong-native-20261013';assert not P.exists() and not pre.exists() and not F.exists();F.mkdir()
scope='All229 strong queries paired with their closest wrong-origin original by precomputed official SSCD square320 embeddings;458native directions, sorted by positive-minus-wrong similarity margin before native execution. Publisher first-four-digit origin retrieval groundtruth excludes the wrong original; not a representative random negative distribution or proof all different-photo meanings. Frozen2696-file historical native4096/opposite1m/25bank caller, source_tolerance2.0 for all cases as prior semantic-negative campaign. New additive geometry APIs and latest root lifecycle changes excluded; no SSCD inference in native decision, threshold tuning, current-root parity or hard RSS cap.'
pre.write_text(json.dumps(dict(plan=plan,pins=pins,scope=scope),indent=2)+'\n');pins[str(pre)]=sha(pre)
d=dict(status='running',producer_pid=os.getpid(),source=str(S),source_manifest=base['source_manifest'],executable=str(binary),plan=plan,pins=pins,completed=0,required=len(plan),results=[],counts=dict(false_acceptance=0,inconclusive=0,rejected=0,execution_failed=0),scope=scope)
def save():
 t=P.with_suffix('.tmp');t.write_text(json.dumps(d,indent=2)+'\n');t.replace(P)
save()
try:
 for i,r in enumerate(plan):
  for p in [r['left'],r['right'],str(binary),str(weights)]:assert sha(p)==pins[p]
  cmd=['/usr/bin/time','-l','/usr/bin/env','DYLD_LIBRARY_PATH='+str(T/'rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib'),str(binary),r['left'],r['right'],str(weights),'640','2.0','1000000'];start=time.monotonic();child=subprocess.Popen(cmd,cwd='/tmp',stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);d.update(active_index=i,child_pid=child.pid);save();stdout,stderr=child.communicate();kind='execution_failed'
  case=dict(index=i,command=cmd,returncode=child.returncode,stdout=stdout,stderr=stderr,elapsed_seconds=time.monotonic()-start)
  if child.returncode==0:
   v=json.loads(stdout);assert v['files']==[2,9] and v['managed_used']==0 and v['opposite_trials']==1000000 and len(v['pairs'])==1;p=v['pairs'][0];assert (p['left'],p['right'])==(2,9);support=p['support'];assert support is None or type(support) is bool;kind='false_acceptance' if support is True else ('rejected' if support is False else 'inconclusive');case['rss_bytes']=int(re.search(r'(\d+)\s+maximum resident set size',stderr)[1])
  for p in [r['left'],r['right'],str(binary),str(weights)]:assert sha(p)==pins[p]
  case['status']=kind;out=F/f'{i:04}.json';assert not out.exists();out.write_text(json.dumps(case,indent=2)+'\n');d['results'].append(dict(index=i,status=kind,report=str(out),sha256=sha(out)));d['counts'][kind]+=1;d['completed']=i+1;save();print(i,kind,flush=True)
 assert all(sha(p)==h for p,h in pins.items()) and all(sha(S/p)==h for p,h in manifest['source_hashes'].items());d['status']='complete_fixed458_nearest_wrong_directions';save()
except Exception as error:d.update(status='terminal_controller_failure',error=str(error));save();raise
