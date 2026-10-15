#!/usr/bin/env python3
from pathlib import Path
import json,hashlib,subprocess,os,re,shutil
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
M=D/'dedup-committed-build-source-415443172-v2-20261015.json';m=json.loads(M.read_text());F=Path(m['source']);O=D/'dedup-complete-collection-baseline-v2-20261015.json';L=Path('/private/tmp/rrrah-complete-collection-baseline-v2-20261015');assert not O.exists();L.mkdir(exist_ok=False)
for row in m['source_files']:assert sha(F/row['path'])==row['sha256']
B=D/'dedup-collection-fused-preverify-v4-opt3-benchmark-audit-20261015.json';assert B.exists(),'No heavy build until performance benchmark audited'
cmd=['cargo','+stable','test','--locked','--offline','-p','rrrah-decode','-p','rrrah-dedup','--features','rrrah-dedup/decode','--lib','--tests','--no-fail-fast','--jobs','2','--config','profile.test.lto="off"','--target-dir','/private/tmp/rrrah-consensus-collection-target-20261014']
d=dict(status='starting',producer_pid=os.getpid(),producer_sha256=sha(__file__),source_manifest=str(M),source_manifest_sha256=sha(M),source=str(F),command=cmd,compiler=subprocess.check_output(['rustc','+stable','-Vv'],text=True),scope='All lib and integration targets of both decoder and dedup packages on immutable415baseline, decode enabled; no filters or target pruning. Diagnostic noLTO, MacARM only, not allfeatures/MSRV/platform/current Root proof.')
def save():
 t=O.with_suffix('.tmp');t.write_text(json.dumps(d,indent=2)+'\n');t.replace(O)
save();log=L/'cargo.log'
with log.open('x')as out:
 child=subprocess.Popen(cmd,cwd=F,stdout=out,stderr=subprocess.STDOUT);d.update(status='running',child_pid=child.pid);save();rc=child.wait()
d.update(returncode=rc,status='terminal_failure'if rc else'terminal_pending_inventory_audit',cargo_log=str(log),cargo_log_sha256=sha(log));save()
for row in m['source_files']:assert sha(F/row['path'])==row['sha256']
body=log.read_text();paths=re.findall(r'^\s*Running .+ \((/[^\n]+)\)$',body,re.M);d['retained_binaries']=[]
for i,path in enumerate(dict.fromkeys(paths)):
 p=Path(path);copy=L/f'binary-{i}-{p.name}';shutil.copyfile(p,copy);copy.chmod(0o755);assert sha(p)==sha(copy)
 listed=subprocess.run([str(copy),'--list'],capture_output=True,text=True);inventory=L/f'inventory-{i}.log';inventory.write_text(listed.stdout+listed.stderr);d['retained_binaries'].append(dict(original=str(p),copy=str(copy),sha256=sha(copy),inventory=str(inventory),inventory_sha256=sha(inventory),inventory_returncode=listed.returncode))
save();print('Combined full baseline returncode',rc,'retained binaries',len(d['retained_binaries']));raise SystemExit(rc)
