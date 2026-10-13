#!/usr/bin/env python3
import json,hashlib,subprocess,time,os,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
P=D/'dedup-bank-heldout-plan-20261013.json';plan=json.loads(P.read_text());H=Path(plan['harness']);A=Path(plan['source_audit']);a=json.loads(A.read_text());S=Path(a['source'])
assert all(sha(S/p)==h for p,h in a['source_sha256'].items()) and all(sha(H/p)==h for p,h in plan['harness_sha256'].items())
assert sha(plan['bank_audit'])==plan['bank_audit_sha256']
for c in plan['cases']:assert sha(c['plan'])==c['plan_sha256'] and sha(c['case_report'])==c['case_report_sha256']
O=D/'dedup-bank-heldout-20261013.json';L=T/'rrrah-bank-heldout-build-20261013.log';F=T/'rrrah-bank-heldout-qualified-20261013';assert not O.exists() and not L.exists() and not F.exists();F.mkdir()
d=dict(status='building',producer_pid=os.getpid(),plan_sha256=sha(P),source_audit_sha256=sha(A),required=len(plan['cases']),completed=0,results=[],build_log=str(L),scope=plan['scope'])
def save():
 q=O.with_suffix('.tmp');q.write_text(json.dumps(d,indent=2)+'\n');q.replace(O)
save()
with L.open('x') as log:
 for cmd in [['cargo','generate-lockfile','--offline'],['cargo','build','--offline','--locked','--jobs','2','--target-dir',str(T/'rrrah-dedup-exact-portfolio-20261011')]]:
  d['active_command']=cmd;p=subprocess.Popen(cmd,cwd=H,stdout=log,stderr=subprocess.STDOUT);d['child_pid']=p.pid;save();code=p.wait()
  if code:d.update(status='terminal_build_failure',returncode=code,build_log_sha256=sha(L));save();raise SystemExit(code)
d.update(build_log_sha256=sha(L),lock_sha256=sha(H/'Cargo.lock'));exe=F/'bank_heldout_diagnostic';subprocess.run(['cp','-cp',str(T/'rrrah-dedup-exact-portfolio-20261011/debug/bank_heldout_diagnostic'),str(exe)],check=True);d.update(status='running',executable=str(exe),executable_sha256=sha(exe));save()
for c in plan['cases']:
 assert sha(c['plan'])==c['plan_sha256'] and sha(c['case_report'])==c['case_report_sha256'] and sha(exe)==d['executable_sha256']
 cmd=['/usr/bin/time','-l',str(exe),c['plan']];d.update(active_label=c['label'],active_command=cmd);p=subprocess.Popen(cmd,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);d['child_pid']=p.pid;save();start=time.monotonic();stdout,stderr=p.communicate();elapsed=time.monotonic()-start
 assert sha(c['case_report'])==c['case_report_sha256'];v=json.loads(stdout) if p.returncode==0 else None;rss=re.search(r'(\d+)\s+maximum resident set size',stderr);row=dict(label=c['label'],returncode=p.returncode,stdout=stdout,stderr=stderr,elapsed_seconds=elapsed,rss_bytes=int(rss[1]) if rss else None);out=F/(c['label']+'.json');out.write_text(json.dumps(row,indent=2)+'\n');d['results'].append(dict(label=c['label'],report=str(out),report_sha256=sha(out),terminal=v['result']['terminal'] if v else 'execution_failed'));d['completed']+=1;save();print(c['label'],d['results'][-1]['terminal'],flush=True)
d.update(status='complete_fixed12_bank_heldout',source_unchanged=all(sha(S/p)==h for p,h in a['source_sha256'].items()),harness_unchanged=all(sha(H/p)==h for p,h in plan['harness_sha256'].items()));save()
