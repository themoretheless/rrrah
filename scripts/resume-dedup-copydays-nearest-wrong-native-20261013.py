#!/usr/bin/env python3
import json,hashlib,os,subprocess,time,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp');sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();old=D/'dedup-copydays-nearest-wrong-native-20261013.json';d=json.loads(old.read_text());assert d['completed']==len(d['results'])==59
for key in ['producer_pid','child_pid']:
 try:os.kill(d[key],0)
 except ProcessLookupError:pass
 else:raise RuntimeError('Original handle still exists; do not resume concurrently')
audit=D/'dedup-copydays-nearest-wrong-native-prefix-0059-audit-20261013.json';a=json.loads(audit.read_text());assert a['completed']==59 and a['counts']==d['counts'];assert all(sha(p)==h for p,h in d['pins'].items());assert all(sha(r['report'])==r['sha256'] for r in d['results']);S=Path(d['source']);manifest=json.loads(Path(d['source_manifest']).read_text());assert all(sha(S/p)==h for p,h in manifest['source_hashes'].items());weights=next(p for p in d['pins'] if p.endswith('loftr-outdoor-native-20261010.safetensors'));binary=d['executable'];P=D/'dedup-copydays-nearest-wrong-native-resume-20261013.json';F=T/'rrrah-copydays-nearest-wrong-native-resume-20261013';assert not P.exists() and not F.exists();F.mkdir();capture=D/'dedup-copydays-nearest-wrong-native-stopped-capture-20261013.json';assert not capture.exists();capture.write_text(json.dumps(dict(status='original_producer_and_child_handles_missing_exit_status_unknown',original_report_sha256=sha(old),producer_pid=d['producer_pid'],child_pid=d['child_pid'],completed=59,active_index=d['active_index'],verified_prefix_audit_sha256=sha(audit),unfinished_case_result_exists=False,cause='Not established; no exit-status/session output available. Original handles absent on repeated live OS checks, not merely observation timeout.'),indent=2)+'\n');d['pins'].update({str(p):sha(p) for p in [old,audit,capture,Path(__file__)]});d.update(status='running',producer_pid=os.getpid(),resume_from=59,original_report=str(old),scope=d['scope']+' Resume after both original process handles were confirmed missing; first59resultfiles retained byte-for-byte and independently audited. Original458plan and frozen source/runtime/policy unchanged.');d.pop('child_pid',None)
def save():
 p=P.with_suffix('.tmp');p.write_text(json.dumps(d,indent=2)+'\n');p.replace(P)
save()
try:
 for i in range(59,len(d['plan'])):
  r=d['plan'][i]
  for p in [r['left'],r['right'],binary,weights]:assert sha(p)==d['pins'][p]
  cmd=['/usr/bin/time','-l','/usr/bin/env','DYLD_LIBRARY_PATH='+str(T/'rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib'),binary,r['left'],r['right'],weights,'640','2.0','1000000'];start=time.monotonic();child=subprocess.Popen(cmd,cwd='/tmp',stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);d.update(active_index=i,child_pid=child.pid);save();stdout,stderr=child.communicate();kind='execution_failed';case=dict(index=i,command=cmd,returncode=child.returncode,stdout=stdout,stderr=stderr,elapsed_seconds=time.monotonic()-start)
  if child.returncode==0:
   v=json.loads(stdout);assert v['files']==[2,9] and v['managed_used']==0 and v['opposite_trials']==1000000 and len(v['pairs'])==1;pair=v['pairs'][0];assert (pair['left'],pair['right'])==(2,9);support=pair['support'];assert support is None or type(support)is bool;kind='false_acceptance' if support is True else ('rejected' if support is False else 'inconclusive');case['rss_bytes']=int(re.search(r'(\d+)\s+maximum resident set size',stderr)[1])
  for p in [r['left'],r['right'],binary,weights]:assert sha(p)==d['pins'][p]
  case['status']=kind;out=F/f'{i:04}.json';assert not out.exists();out.write_text(json.dumps(case,indent=2)+'\n');d['results'].append(dict(index=i,status=kind,report=str(out),sha256=sha(out)));d['counts'][kind]+=1;d['completed']=i+1;save();print(i,kind,flush=True)
 assert all(sha(p)==h for p,h in d['pins'].items()) and all(sha(S/p)==h for p,h in manifest['source_hashes'].items());d['status']='complete_fixed458_nearest_wrong_directions';save()
except Exception as error:d.update(status='terminal_controller_failure',error=str(error));save();raise
