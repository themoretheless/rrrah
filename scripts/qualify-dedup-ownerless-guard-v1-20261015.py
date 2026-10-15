#!/usr/bin/env python3
from pathlib import Path
import json,hashlib,subprocess,shutil,os,re
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();F=Path('/private/tmp/rrrah-ownerless-guard-v1-20261015');F.mkdir(exist_ok=False);src=R/'crates/rrrah-dedup/src/collection.rs';text=src.read_text();start=text.index('fn stopped(');end=text.index('\ntrait Observation',start);original=F/'original-guards.rs';original.write_text(text[start:end]);H=D/'dedup-ownerless-guard-controls-v1-20261015.rs';shutil.copyfile(H,F/'probe.rs');M=D/'dedup-actual-collection-metadata-v1-20261015.json';m=json.loads(M.read_text());dep=next(k for k in m['dependency_pins']if Path(k).name.startswith('librrrah_decode-'));assert sha(dep)==m['dependency_pins'][dep];T=Path('/private/tmp/rrrah-consensus-collection-target-20261014/debug');exe=F/'tests';cmd=['rustc','+stable','--edition=2024','--test',str(F/'probe.rs'),'-C','opt-level=3','-L','dependency='+str(T/'deps'),'--extern','rrrah_decode='+dep,'-o',str(exe)]
for pattern,subdir in [('blake3-*','out'),('astcenc-sys-*','out'),('basis-universal-sys-*','out'),('charls-sys-*','out/lib'),('link-cplusplus-*','out'),('openjpeg-sys-*','out'),('jpegxr-*','out')]:
 for p in (T/'build').glob(pattern):
  if(p/subdir).is_dir():cmd+=['-L','native='+str(p/subdir)]
cmd+=['-L','native=/opt/homebrew/opt/dav1d/lib','-L','native=/opt/homebrew/opt/libheif/lib'];O=D/'dedup-ownerless-guard-v1-20261015.json';assert not O.exists();d=dict(status='starting',producer_pid=os.getpid(),producer_sha256=sha(__file__),source_sha256=sha(src),original_fragment_sha256=sha(original),harness_sha256=sha(H),dependency_pins={dep:sha(dep)},command=cmd,scope='Unchanged current stopped/owners_stopped functions, real cached rrrah_decode DecodeRequest/GenerationToken types; candidate only passes emptyslice when all immutable requests lackowner tokens. Component correctness/control counting, no whole collection/provider/decode/runtime or Root adoption proof.')
def save():
 t=O.with_suffix('.tmp');t.write_text(json.dumps(d,indent=2)+'\n');t.replace(O)
for label,command in [('build',cmd),('list',[str(exe),'--list']),('run',[str(exe),'--test-threads','1'])]:
 with(F/(label+'.log')).open('x')as out:
  child=subprocess.Popen(command,cwd=R,stdout=out,stderr=subprocess.STDOUT);d.update(status=label,child_pid=child.pid);save();rc=child.wait()
 d[label+'_returncode']=rc;save()
 if rc:d.update(status='terminal_'+label+'_failure');save();raise SystemExit(rc)
body=(F/'run.log').read_text();assert '2 passed; 0 failed; 1 ignored'in body and sha(src)==d['source_sha256']and sha(H)==sha(F/'probe.rs')==d['harness_sha256'];assert sha(dep)==d['dependency_pins'][dep];d.update(status='passed_pending_independent_audit',passed=2,ignored=1,executable=str(exe),executable_sha256=sha(exe),log_pins={str(p):sha(p)for p in F.glob('*.log')});save();print('2originalguard realtype equivalence tests passed;1manual benchmark ignored')
