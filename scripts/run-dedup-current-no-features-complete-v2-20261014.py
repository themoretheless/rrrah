#!/usr/bin/env python3
"""Complete current no-feature lib/integration gate; same process checks, no skips."""
from pathlib import Path
import json,hashlib,subprocess,os
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();O=D/'dedup-current-no-features-complete-v2-20261014.json';assert not O.exists();F=Path('/private/tmp/rrrah-current-no-features-complete-v2-20261014');F.mkdir(exist_ok=False)
paths=sorted((R/'crates/rrrah-dedup').rglob('*.rs'))+[R/'Cargo.toml',R/'Cargo.lock',R/'crates/rrrah-dedup/Cargo.toml'];pins={str(p.relative_to(R)):sha(p)for p in paths};M=D/'dedup-current-no-features-complete-v2-source-20261014.json';assert not M.exists();M.write_text(json.dumps(dict(source=str(R),source_sha256=pins,scope='Current root dedup Rust sources plus root manifest/lock/package manifest; no-feature compile/test scope. Optional dependency source bodies not compiled or qualified.'),indent=2)+'\n');cmd=['cargo','test','--locked','-p','rrrah-dedup','--no-default-features','--lib','--tests','--no-fail-fast','--jobs','2'];L=F/'cargo.log';d=dict(status='running',producer_pid=os.getpid(),source=str(R),source_manifest=str(M),source_manifest_sha256=sha(M),producer_sha256=sha(__file__),command=cmd,log=str(L),scope='Complete current root no-default-feature library and all declared integration targets. Includes current Mac16gradient and transparent-texture pixel regression. Permissioned ps observes own synthetic descendants; no process-test weakening/skips. Decode/raster/LoFTR/allfeatures/platform/format/corpus/workspace not qualified.')
def save():
 t=O.with_suffix('.tmp');t.write_text(json.dumps(d,indent=2)+'\n');t.replace(O)
save()
with L.open('x')as f:
 c=subprocess.Popen(cmd,cwd=R,stdout=f,stderr=subprocess.STDOUT);d['child_pid']=c.pid;save();rc=c.wait()
d.update(returncode=rc,status='passed_pending_inventory_audit'if rc==0 else'terminal_test_or_compile_failure',log_sha256=sha(L),source_unchanged=all(sha(R/k)==h for k,h in pins.items()));save();print(d['status'])
