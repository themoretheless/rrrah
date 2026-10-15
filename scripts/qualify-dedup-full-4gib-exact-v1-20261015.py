#!/usr/bin/env python3
from pathlib import Path
import subprocess,json,hashlib,shutil,os,re
R=Path(__file__).resolve().parents[1];D=R/'docs/research';F=Path('/private/tmp/rrrah-full-4gib-exact-v1-20261015');F.mkdir(exist_ok=False);sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();source=R/'crates/rrrah-dedup/src/exact.rs';h=D/'dedup-full-4gib-exact-v1-20261015.rs';shutil.copyfile(source,F/'exact.rs');shutil.copyfile(h,F/'probe.rs');prior=json.loads((D/'dedup-gradient-unroll4-managed-focused-v2-20261014.json').read_text());deps={k:v for k,v in prior['dependency_pins'].items()if any('/lib'+n+'-'in k for n in ['blake3','thiserror','tempfile','libc'])};assert len(deps)==4 and all(sha(k)==v for k,v in deps.items());exe=F/'probe';cmd=['rustc','+stable','--edition=2024',str(F/'probe.rs'),'-C','opt-level=3','-L','dependency='+str(Path(next(iter(deps))).parent),'-o',str(exe)]
for k in deps:cmd+=['--extern',Path(k).name.split('-',1)[0][3:]+'='+k]
O=D/'dedup-full-4gib-exact-v1-20261015.json';assert not O.exists();d=dict(status='starting',producer_pid=os.getpid(),producer_sha256=sha(__file__),source_sha256=sha(source),harness_sha256=sha(h),dependency_pins=deps,command=cmd,compiler=subprocess.check_output(['rustc','+stable','-Vv'],text=True),scope='Current exact module standalone real cached dependencies on MacARM, actual full public compare_files streaming of two owned sparse synthetic files each4GiB+64KiB. Equal case and one late byte difference beyond4GiB. No physical-device IO throughput, hard RSS, Windows/Linux or collection/decode/fullworkspace proof; timing diagnostic only under concurrent compilation.')
def save():
 t=O.with_suffix('.tmp');t.write_text(json.dumps(d,indent=2)+'\n');t.replace(O)
save()
for label,command in [('build',cmd),('run',[str(exe)])]:
 with (F/(label+'.log')).open('x')as log:
  child=subprocess.Popen(command,cwd=R,stdout=log,stderr=subprocess.STDOUT);d.update(status=label,child_pid=child.pid);save();rc=child.wait()
 d[label+'_returncode']=rc;save()
 if rc:d.update(status='terminal_'+label+'_failure');save();raise SystemExit(rc)
text=(F/'run.log').read_text();fields=dict(re.findall(r'([A-Z_0-9]+)=(\S+)',text));assert fields['LENGTH']=='4295032832'and fields['EQUAL']=='true'and fields['LATE_DIFFERENT']=='false'and int(fields['CHECKPOINTS'])>0
assert sha(source)==sha(F/'exact.rs')==d['source_sha256']and sha(h)==sha(F/'probe.rs')==d['harness_sha256']and all(sha(k)==v for k,v in deps.items());d.update(status='passed_pending_independent_audit',results=fields,executable_sha256=sha(exe),log_pins={str(p):sha(p)for p in F.glob('*.log')});save();print(fields)
