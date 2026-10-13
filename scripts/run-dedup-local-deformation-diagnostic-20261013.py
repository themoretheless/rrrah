#!/usr/bin/env python3
import json,hashlib,subprocess,os
from pathlib import Path
R=Path(__file__).resolve().parents[1];T=Path('/Users/themoretheless/.codex/tmp');D=R/'docs/research';P=D/'dedup-local-deformation-diagnostic-plan-20261013.json';O=D/'dedup-local-deformation-diagnostic-20261013.json';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
p=json.loads(P.read_text());h=Path(p['harness']);audit=R/p['source_audit'];assert sha(audit)==p['source_audit_sha256'];a=json.loads(audit.read_text());S=Path(a['source']);assert all(sha(S/k)==v for k,v in a['source_sha256'].items());assert all(sha(h/k)==v for k,v in p['harness_sha256'].items());dest=T/'rrrah-local-deformation-diagnostic-qualified-20261013';dest.mkdir();exe=dest/'local_deformation_diagnostic';subprocess.run(['cp','-cp',str(T/'rrrah-dedup-exact-portfolio-20261011/debug/local_deformation_diagnostic'),str(exe)],check=True)
pins={str(P):sha(P),str(audit):sha(audit),str(exe):sha(exe),str(h/'Cargo.lock'):sha(h/'Cargo.lock'),str(Path(__file__)):sha(__file__)}
for c in p['cases']:
 q=Path(c['plan']);q=q if q.is_absolute() else R/q;assert sha(q)==c['plan_sha256'];pins[str(q)]=sha(q)
 for key in ['left','right']:
  v=Path(json.loads(q.read_text())[key]);pins[str(v)]=sha(v)
d=dict(status='running',producer_pid=os.getpid(),pins=pins,source_audit=str(audit),source_files=len(a['source_sha256']),cases=[],scope=p['scope'])
def save():
 q=O.with_suffix('.tmp');q.write_text(json.dumps(d,indent=2)+'\n');q.replace(O)
assert not O.exists();save()
for i,c in enumerate(p['cases']):
 q=Path(c['plan']);q=q if q.is_absolute() else R/q;cmd=[str(exe),str(q)];r=subprocess.run(cmd,cwd=R,capture_output=True,text=True);out=dest/('%02d.json'%i);out.write_text(r.stdout);err=dest/('%02d.stderr'%i);err.write_text(r.stderr);row=dict(label=c['label'],command=cmd,returncode=r.returncode,result=str(out),result_sha256=sha(out),stderr_sha256=sha(err));assert r.returncode==0,r.stderr;v=json.loads(r.stdout);assert v['managed_used']==0;row['directions']=v['directions'];d['cases'].append(row);save()
assert all(sha(k)==v for k,v in pins.items());assert all(sha(S/k)==v for k,v in a['source_sha256'].items());d['status']='complete_pins_verified_pending_interpretation';save();print(d['status'])
