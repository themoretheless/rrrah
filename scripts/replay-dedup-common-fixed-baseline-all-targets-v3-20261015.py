"""Replay every retained target after ENOSPC; preserve failed Cargo gate verbatim."""
from pathlib import Path
import hashlib,json,os,subprocess,time
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
original_path=D/'dedup-common-worker-fixed-baseline-gate-v2-20261015.json';original=json.loads(original_path.read_text())
failed_path=D/'dedup-common-worker-fixed-baseline-audit-v2-20261015.json';failed=json.loads(failed_path.read_text())
assert original['status']=='terminal_failure' and failed['failed']==9 and not failed['gate_passed'] and len(original['retained_binaries'])==162
assert failed['producer_report_sha256']==h(original_path)
manifest=Path(original['manifest']);assert h(manifest)==original['manifest_sha256'];prepared=json.loads(manifest.read_text());snapshot=next(v for v in prepared['variants'] if v['variant']=='baseline');source=Path(snapshot['source'])
for row in snapshot['source_files']:assert h(source/row['path'])==row['sha256']
for row in original['retained_binaries']:
 assert h(row['copy'])==row['sha256'] and h(row['inventory'])==row['inventory_sha256']
work=Path('/private/tmp/rrrah-common-fixed-baseline-replay-v3-20261015');assert not work.exists();work.mkdir();tmp=work/'tmp';tmp.mkdir()
output=D/'dedup-common-fixed-baseline-replay-v3-20261015.json';assert not output.exists()
data=dict(status='running',producer_pid=os.getpid(),producer_sha256=h(__file__),original_report=str(original_path),original_report_sha256=h(original_path),failed_audit=str(failed_path),failed_audit_sha256=h(failed_path),manifest=str(manifest),manifest_sha256=h(manifest),source=str(source),targets=[],tmpdir=str(tmp),scope='All162 actual retained lib/integration executables from full frozen baseline Cargo gate, same machine code and original test inventory, no filters/pruning/recompilation. Package cwd and CARGO_MANIFEST_DIR explicit. TMPDIR redirected to owned directory on same filesystem after releasing only rebuildable incremental cache. Original ENOSPC gate preserved failed. Not current mutable Root/platform/corpus/performance qualification.')
def save():
 stage=output.with_suffix('.tmp');stage.write_text(json.dumps(data,indent=2)+'\n');stage.replace(output)
save()
for i,row in enumerate(original['retained_binaries']):
 package='rrrah-decode' if Path(row['original']).name.startswith('rrrah_decode-') else 'rrrah-dedup'
 # Only the first library target belongs to decode; all remaining package
 # targets are resolved against the original cargo Running header below.
 logtext=Path(original['cargo_log']).read_text()
 import re
 header=next(m.group(1) for m in re.finditer(r'^\s*Running (.+) \((/[^\n]+)\)$',logtext,re.M) if m.group(2)==row['original'])
 if header.startswith('unittests src/lib.rs'):package='rrrah-decode' if Path(row['original']).name.startswith('rrrah_decode-') else 'rrrah-dedup'
 else:
  relative=header.removeprefix('tests/') if hasattr(header,'removeprefix') else header[6:]
  candidates=[p for p in ('rrrah-decode','rrrah-dedup') if (source/'crates'/p/'tests'/relative).is_file()]
  assert len(candidates)==1,(header,candidates);package=candidates[0]
 cwd=source/'crates'/package;env=dict(os.environ,TMPDIR=str(tmp),CARGO_MANIFEST_DIR=str(cwd))
 command=[row['copy']];log=work/f'target-{i:03}.log';start=time.time()
 with log.open('x') as stream:
  child=subprocess.Popen(command,cwd=cwd,env=env,stdout=stream,stderr=subprocess.STDOUT)
  data.update(current_target=i,current_child_pid=child.pid);save();code=child.wait()
 data['targets'].append(dict(index=i,original=row['original'],binary=row['copy'],binary_sha256=row['sha256'],inventory=row['inventory'],inventory_sha256=row['inventory_sha256'],package=package,cwd=str(cwd),command=command,returncode=code,log=str(log),log_sha256=h(log),diagnostic_elapsed_seconds=time.time()-start));save()
for row in snapshot['source_files']:assert h(source/row['path'])==row['sha256']
assert len(data['targets'])==162
data.update(status='terminal_pending_audit',returncode=0 if all(r['returncode']==0 for r in data['targets']) else 1);save();print('Complete replay terminal:',data['returncode'],'targets',len(data['targets']))
