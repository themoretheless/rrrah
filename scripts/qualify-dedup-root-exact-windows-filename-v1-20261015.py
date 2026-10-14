#!/usr/bin/env python3
from pathlib import Path
import hashlib,json,subprocess,os,re,shutil
R=Path(__file__).resolve().parents[1];D=R/'docs/research';F=Path('/private/tmp/rrrah-root-exact-windows-filename-v1-20261015');F.mkdir(exist_ok=False);sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();prior=json.loads((D/'dedup-gradient-unroll4-managed-focused-v2-20261014.json').read_text());deps={k:v for k,v in prior['dependency_pins'].items()if any('/lib'+n+'-'in k for n in ['blake3','thiserror','tempfile','libc'])};assert len(deps)==4 and all(sha(k)==v for k,v in deps.items());pins={}
for source,name in [(R/'crates/rrrah-dedup/src/exact.rs','exact.rs'),(R/'crates/rrrah-dedup/tests/exact_large_files.rs','controls.rs'),(R/'crates/rrrah-dedup/tests/exact_files.rs','integration.rs')]:
 shutil.copyfile(source,F/name);assert sha(source)==sha(F/name);pins[str(source.relative_to(R))]=sha(source)
h=F/'harness.rs';h.write_text('extern crate self as rrrah_dedup;\n#[path="exact.rs"]pub mod exact;\n#[path="controls.rs"]mod large_controls;\n#[path="integration.rs"]mod integration_controls;\n');exe=F/'tests';cmd=['rustc','+stable','--edition=2024','--test',str(h),'-C','opt-level=0','-L','dependency='+str(Path(next(iter(deps))).parent),'-o',str(exe)]
for k in deps:
 name=Path(k).name.split('-',1)[0][3:];cmd+=['--extern',name+'='+k]
O=D/'dedup-root-exact-windows-filename-v1-20261015.json';assert not O.exists();A=D/'dedup-fused-exact-root-adoption-audit-20261014.json';ad=json.loads(A.read_text());d=dict(adoption_audit=str(A),adoption_audit_sha256=sha(A),status='building',producer_pid=os.getpid(),source_pins=pins,dependency_pins=deps,producer_sha256=sha(__file__),harness_sha256=sha(h),source=str(F),command=cmd,scope='Current Root exact module after portable extensionless fixture fix; opt0 semantic gate, not performance proof plus complete exact_files integration and large-file controls; directrustc real cached dependencies. 4GiB/1TiB sparse metadata/size admission and bounded early streaming cancellation only, not full multiGiB equality, physical IO, hard RSS, otherplatform or Cargo/workspace/decode proof. Existing exact-module unit controls also included. Temporary sparse files owned by tempfile and removed on completion, no corpus.')
def save():
 t=O.with_suffix('.tmp');t.write_text(json.dumps(d,indent=2)+'\n');t.replace(O)
save()
for label,command in [('build',cmd),('inventory',[str(exe),'--list']),('tests',[str(exe),'--test-threads','2'])]:
 with (F/(label+'.log')).open('x')as f:
  child=subprocess.Popen(command,cwd=R,stdout=f,stderr=subprocess.STDOUT);d.update(status=label,child_pid=child.pid);save();rc=child.wait()
 d[label+'_returncode']=rc;save()
 if rc:d.update(status='terminal_'+label+'_failure');save();raise SystemExit(rc)
body=(F/'tests.log').read_text();summary=re.search(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;',body);assert summary and summary[2]=='0';names=set(re.findall(r'^(.+): test$',(F/'inventory.log').read_text(),re.M));results=dict(re.findall(r'^test (.+) \.\.\. (ok|ignored(?:,.*)?)$',body,re.M));assert names==set(results)and len(names)==int(summary[1])+int(summary[3]);assert all(sha(R/k)==v for k,v in pins.items())and all(sha(k)==v for k,v in deps.items());d.update(status='passed_pending_independent_audit',passed=int(summary[1]),ignored=int(summary[3]),failed=0,executable=str(exe),executable_sha256=sha(exe),output_pins={str(p):sha(p)for p in F.glob('*.log')});save();print(d['passed'],'exact and large sparse tests passed')
