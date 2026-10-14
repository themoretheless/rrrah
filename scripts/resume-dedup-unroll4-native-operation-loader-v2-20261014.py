#!/usr/bin/env python3
"""Reuse already built binaries; restore DYLD after protected time via env argv."""
from pathlib import Path
import hashlib,json,os,subprocess,resource,time,signal
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
prior_path=D/'dedup-unroll4-native-operation-20261014.json';prior=json.loads(prior_path.read_text());assert prior['status']=='stopped_on_failure'and not prior['results']and len(prior['builds'])==2 and 'Library not loaded'in prior['error']
P=Path(prior['plan']);assert sha(P)==prior['plan_sha256'];plan=json.loads(P.read_text());assert all(sha(p)==h for p,h in plan['pins'].items())
for v in plan['versions'].values():
 M=Path(v['source_manifest']);assert sha(M)==v['source_manifest_sha256'];m=json.loads(M.read_text());assert all(sha(Path(m['source'])/p)==h for p,h in m['source_sha256'].items())and all(sha(Path(v['harness'])/p)==h for p,h in v['harness_pins'].items())
runtime='/Users/themoretheless/.codex/tmp/rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib';runtime_path=D/'dedup-unroll4-native-operation-runtime-before-20261014.json';rt=json.loads(runtime_path.read_text());assert all(sha(p)==h for p,h in rt['pins'].items())
F=Path('/private/tmp/rrrah-unroll4-native-operation-loader-v2-20261014');F.mkdir(exist_ok=False);builds=[]
for b in prior['builds']:
 assert sha(b['executable'])==b['executable_sha256']and sha(b['log'])==b['log_sha256'];folder=F/b['version'];folder.mkdir();binary=folder/'probe';subprocess.run(['cp','-cp',b['executable'],str(binary)],check=True);assert sha(binary)==b['executable_sha256'];builds.append(dict(**{k:v for k,v in b.items()if k!='executable'},executable=str(binary)))
# Two cheap observations: wrapped native usage reaches main, and ordinary timing exits0.
probe=builds[0]['executable'];usage_command=['/usr/bin/time','/usr/bin/env','DYLD_LIBRARY_PATH='+runtime,probe];usage=subprocess.run(usage_command,capture_output=True,text=True)
assert usage.returncode==1 and 'usage: native_bank_outcome_files' in usage.stderr and 'Library not loaded'not in usage.stderr
clock_command=['/usr/bin/time','/usr/bin/env','DYLD_LIBRARY_PATH='+runtime,'/private/tmp/rrrah-gradient-lanes16-public-20261014/baseline/probe','--describe'];clock=subprocess.run(clock_command,capture_output=True,text=True);assert clock.returncode==0 and clock.stdout.strip()=='1040 aarch64 macos'and 'sysctl kern.clockrate'not in clock.stderr
observation=F/'loader-controls.json';observation.write_text(json.dumps(dict(usage=dict(command=usage_command,returncode=usage.returncode,stdout=usage.stdout,stderr=usage.stderr),clock=dict(command=clock_command,returncode=clock.returncode,stdout=clock.stdout,stderr=clock.stderr)),indent=2)+'\n')
O=D/'dedup-unroll4-native-operation-loader-v2-20261014.json';assert not O.exists();state=dict(status='running',producer_pid=os.getpid(),plan=str(P),plan_sha256=sha(P),producer_sha256=sha(__file__),prior_failed_report=str(prior_path),prior_failed_report_sha256=sha(prior_path),first_audit_sha256=prior['first_audit_sha256'],runtime_before=str(runtime_path),runtime_before_sha256=sha(runtime_path),runtime_before_pins=rt['pins'],loader_controls=str(observation),loader_controls_sha256=sha(observation),builds=builds,results=[],scope=plan['scope']+' Reuses both successful binaries after terminal loader failure. /usr/bin/time default clock only, /usr/bin/env restores explicit DYLD after protected executable; no RSS sysctl or binary patch/rebuild. This is earlier V5vsV7dimensionunroll4, not current firstpass16qualification.')
def save():
 tmp=O.with_suffix('.tmp');tmp.write_text(json.dumps(state,indent=2)+'\n');tmp.replace(O)
save();outputs={};child=None
try:
 for case in plan['cases']:
  for pair in range(3):
   for version in (['baseline','candidate']if pair%2==0 else ['candidate','baseline']):
    assert all(sha(p)==plan['pins'][p]for p in case['arguments'][:3]);binary=next(b['executable']for b in builds if b['version']==version);command=['/usr/bin/time','/usr/bin/env','DYLD_LIBRARY_PATH='+runtime,binary,*case['arguments']]
    before=resource.getrusage(resource.RUSAGE_CHILDREN);start=time.monotonic();child=subprocess.Popen(command,cwd=R,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,start_new_session=True);state.update(child_pid=child.pid,active_case=case['label'],active_pair=pair,active_version=version);save();stdout,stderr=child.communicate();code=child.returncode;child=None;after=resource.getrusage(resource.RUSAGE_CHILDREN)
    raw=F/(case['label']+'-'+str(pair)+'-'+version+'.json');raw.write_text(json.dumps(dict(command=command,returncode=code,stdout=stdout,stderr=stderr),indent=2)+'\n');assert code==0,stderr
    result=json.loads(stdout);assert result['managed_used']==0 and result['support']is True and result['forward_support']is True and result['reverse_support']is True
    if case['label']in outputs:assert outputs[case['label']]==result
    else:outputs[case['label']]=result
    state['results'].append(dict(case=case['label'],pair=pair,version=version,report=str(raw),report_sha256=sha(raw),cpu_seconds=after.ru_utime+after.ru_stime-before.ru_utime-before.ru_stime,wall_seconds=time.monotonic()-start,managed_peak=result['managed_peak']));save()
 assert all(sha(p)==h for p,h in plan['pins'].items())and all(sha(p)==h for p,h in rt['pins'].items())
 for v in plan['versions'].values():
  m=json.loads(Path(v['source_manifest']).read_text());assert all(sha(Path(m['source'])/p)==h for p,h in m['source_sha256'].items())
 state['status']='complete_pending_audit';save()
except BaseException as e:
 state.update(status='stopped_on_failure',error=repr(e));save();raise
finally:
 if child is not None and child.poll()is None:os.killpg(child.pid,signal.SIGKILL);child.wait()
