#!/usr/bin/env python3
from pathlib import Path
import subprocess,json,hashlib,os
R=Path(__file__).resolve().parents[1];D=R/'docs/research';M=D/'dedup-maintained-pastey-strict-v2-source-20261015.json';m=json.loads(M.read_text());F=Path(m['source']);sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert all(sha(F/name)==h for name,h in m['source_pins'].items());assert sha(F/'Cargo.toml')==m['workspace_manifest_sha256'];lock=F/'Cargo.lock';assert 'name = "paste"\n'not in lock.read_text()and 'name = "pastey"\n'in lock.read_text();O=D/'dedup-maintained-pastey-strict-v3-20261015.json';assert not O.exists();d=dict(status='running',producer_pid=os.getpid(),producer_sha256=sha(__file__),source_manifest_sha256=sha(M),toolchain={'rustc':subprocess.check_output(['rustc','+stable','-Vv'],cwd=F,text=True),'cargo':subprocess.check_output(['cargo','+stable','-V'],cwd=F,text=True)},lock_sha256=sha(lock),rows=[],scope='Isolated original pulp0.22.3/texture2ddecoder0.1.2 default-feature production library strict -Dwarnings checks, hostMacARM and WindowsGNU crossmetadata.3compiler-suggested explicit returnlifetimes plus pastey alias/globalisolatedpulp patch; no feature removal. Includes unchanged upstream default host library test targets under stable1.98.1 with strict warnings. No Windows linking/execution, Rootintegration, wholecodec/EXR/MSRV1.92/runtime/performance or overalladvisory qualification.')
def save():
 t=O.with_suffix('.tmp');t.write_text(json.dumps(d,indent=2)+'\n');t.replace(O)
env=os.environ.copy();env['RUSTFLAGS']='-Dwarnings';save()
for label,target in [('macos',None),('windows-gnu','x86_64-pc-windows-gnu'),('macos-tests',None)]:
 cmd=['cargo','+stable','check','--locked','--offline','--manifest-path',str(F/'Cargo.toml'),'--workspace','--lib','--jobs','1','--config','profile.dev.debug=0']
 if label=='macos-tests':
  cmd[2]='test';cmd+=['--no-fail-fast','--config','profile.test.opt-level=0','--config','profile.test.debug=0']
 if target:cmd+=['--target',target]
 L=F/(label+'-strict-v3.log')
 with L.open('x')as log:
  child=subprocess.Popen(cmd,cwd=F,env=env,stdout=log,stderr=subprocess.STDOUT);d.update(active=dict(label=label,command=cmd,child_pid=child.pid));save();rc=child.wait()
 d['rows'].append(dict(label=label,command=cmd,returncode=rc,log=str(L),log_sha256=sha(L)));save()
 if rc:d.update(status='terminal_'+label+'_check_failure');save();raise SystemExit(rc)
changes=[name for name,h in m['source_pins'].items()if sha(F/name)!=h];assert not changes and sha(lock)==d['lock_sha256'];d.update(status='passed_pending_independent_audit',source_changes=changes,active=None);save();print('Stable strict host/Windows metadata and host upstream library targets passed')
