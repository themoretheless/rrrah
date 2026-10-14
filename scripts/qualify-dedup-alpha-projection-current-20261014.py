#!/usr/bin/env python3
from pathlib import Path
import json,hashlib,subprocess,os,re
R=Path(__file__).resolve().parents[1];D=R/'docs/research';M=D/'dedup-alpha-projection-current-source-20261014.json';m=json.loads(M.read_text());sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert all(sha(R/k)==h for k,h in m['source_sha256'].items());F=D/'dedup-alpha-projection-current-20261014';F.mkdir(exist_ok=False);O=D/'dedup-alpha-projection-current-20261014.json';assert not O.exists();log=F/'cargo.log';cmd=['cargo','test','--locked','-p','rrrah-dedup','--no-default-features','--test','pixels'];d=dict(status='running',producer_pid=os.getpid(),command=cmd,source_manifest=str(M.relative_to(R)),source_manifest_sha256=sha(M),producer_sha256=sha(__file__),scope=m['scope'])
def save():
 t=O.with_suffix('.tmp');t.write_text(json.dumps(d,indent=2)+'\n');t.replace(O)
save()
with log.open('x')as f:
 c=subprocess.Popen(cmd,cwd=R,stdout=f,stderr=subprocess.STDOUT);d['child_pid']=c.pid;save();rc=c.wait()
d.update(returncode=rc,log_sha256=sha(log),source_unchanged=all(sha(R/k)==h for k,h in m['source_sha256'].items()));save();assert rc==0 and d['source_unchanged']
s=log.read_text();assert '7 passed; 0 failed; 0 ignored' in s
exe=R/re.search(r'Running tests/pixels.rs \(([^\n]+)\)',s)[1];kept=F/'pixels';subprocess.run(['cp','-cp',str(exe),str(kept)],check=True);assert sha(exe)==sha(kept)
v=subprocess.run([str(kept),'--list'],cwd=R,capture_output=True,text=True);assert v.returncode==0;(F/'inventory.log').write_text(v.stdout+v.stderr)
names=set(re.findall(r'^(.+): test$',v.stdout,re.M));executed=dict(re.findall(r'^test (.+) \.\.\. (ok|FAILED|ignored)$',s,re.M));assert len(names)==7 and names==set(executed)and all(v=='ok'for v in executed.values())and m['new_regression']in names
d.update(status='verified_current_root_seven_pixel_tests',passed=7,failed=0,ignored=0,retained_executable=str(kept.relative_to(R)),executable_sha256=sha(kept),inventory_sha256=sha(F/'inventory.log'),tests=sorted(names));save();print('7pixel integration tests verified')
