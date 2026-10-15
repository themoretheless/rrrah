#!/usr/bin/env python3
from pathlib import Path
import subprocess,json,hashlib,os
R=Path(__file__).resolve().parents[1];D=R/'docs/research';F=Path('/private/tmp/rrrah-current-cargo-exact-v2-20261015');F.mkdir(exist_ok=False)
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
paths=list((R/'crates/rrrah-dedup').rglob('*.rs'))+[R/'crates/rrrah-dedup/Cargo.toml',R/'Cargo.toml',R/'Cargo.lock',R/'.cargo/config.toml'];pins={str(p.relative_to(R)):sha(p)for p in paths};cmd=['cargo','test','--locked','--offline','-p','rrrah-dedup','--no-default-features','--lib','--test','exact_files','--test','exact_large_files','--config','profile.test.lto="off"','--jobs','2','--no-fail-fast'];O=D/'dedup-current-cargo-exact-v2-20261015.json';assert not O.exists();d=dict(status='running',producer_pid=os.getpid(),producer_sha256=sha(__file__),source_pins=pins,command=cmd,scope='Actual current Cargo nofeature lib/exact_files/exact_large_files; test LTO explicitly off. No decode/fullworkspace/Windows/release/performance qualification. All dedup Rust files and manifests/config pinned before and after, dependency artifacts not separately pinned.')
def save():
 t=O.with_suffix('.tmp');t.write_text(json.dumps(d,indent=2)+'\n');t.replace(O)
L=F/'cargo.log'
with L.open('x')as log:
 c=subprocess.Popen(cmd,cwd=R,stdout=log,stderr=subprocess.STDOUT);d['child_pid']=c.pid;save();rc=c.wait()
changed=[p for p,h in pins.items()if sha(R/p)!=h];d.update(returncode=rc,source_changes=changed,log=str(L),log_sha256=sha(L),status='passed_pending_independent_audit'if rc==0 and not changed else'terminal_failure_or_source_drift');save();assert rc==0 and not changed;print('Current Cargo exact gate passed')
