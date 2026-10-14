#!/usr/bin/env python3
from pathlib import Path
import json,hashlib,subprocess,os
R=Path(__file__).resolve().parents[1];D=R/'docs/research';M=D/'dedup-maintained-pastey-candidate-source-20261015.json';m=json.loads(M.read_text());F=Path(m['source'])
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
pins={}
for row in m['rows']:
 package=F/row['package']
 for name,h in row['source_file_pins'].items():
  p=package/name;expected=row['patched_manifest_sha256']if name=='Cargo.toml'else h;assert sha(p)==expected;pins[str(p)]=expected
pins[str(F/'Cargo.toml')]=sha(F/'Cargo.toml');pins[str(F/'Cargo.lock')]=sha(F/'Cargo.lock');O=D/'dedup-maintained-pastey-upstream-v2-20261015.json';assert not O.exists();cmd=['cargo','test','--locked','--offline','--manifest-path',str(F/'Cargo.toml'),'--workspace','--lib','--no-fail-fast','--jobs','1','--config','profile.test.opt-level=0','--config','profile.test.debug=0'];d=dict(status='running',producer_pid=os.getpid(),producer_sha256=sha(__file__),source_manifest_sha256=sha(M),source_pins=pins,command=cmd,scope='Original default-feature library test targets for isolated pulp0.22.3 andtexture2ddecoder0.1.2 with only manifest paste->pastey0.2.3 alias changes. No test bodies removed or filtered. macARM opt0semantic gate, no Rootintegration/EXR/fullcodec/x86/Windows/performance qualification. A zero-test target must not count as decoded-format runtime coverage.')
def save():
 t=O.with_suffix('.tmp');t.write_text(json.dumps(d,indent=2)+'\n');t.replace(O)
L=F/'upstream-tests-v2.log'
with L.open('x')as log:
 c=subprocess.Popen(cmd,cwd=F,stdout=log,stderr=subprocess.STDOUT);d['child_pid']=c.pid;save();rc=c.wait()
changed=[p for p,h in pins.items()if sha(p)!=h];d.update(returncode=rc,source_changes=changed,log=str(L),log_sha256=sha(L),status='passed_pending_independent_audit'if rc==0 and not changed else'terminal_failure_or_source_drift');save();assert rc==0 and not changed;print('Original isolated upstream test targets completed')
