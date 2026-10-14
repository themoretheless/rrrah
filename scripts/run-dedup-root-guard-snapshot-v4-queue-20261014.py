#!/usr/bin/env python3
"""Wait for the existing Root gate, then qualify immutable snapshot without restarting it."""
from pathlib import Path
import json,hashlib,subprocess,sys,time,os
R=Path(__file__).resolve().parents[1];D=R/'docs/research';P=D/'dedup-root-guard-latch-decode-20261014.json';O=D/'dedup-root-guard-snapshot-v4-queue-20261014.json';assert not O.exists();capture=D/'dedup-root-guard-snapshot-v4-20261014.json';C=json.loads(capture.read_text());assert C['status']=='stable_snapshot_captured';d=dict(status='waiting_existing_root_gate',producer_pid=os.getpid(),capture_sha256=hashlib.sha256(capture.read_bytes()).hexdigest(),producer_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest())
def save():
 t=O.with_suffix('.tmp');t.write_text(json.dumps(d,indent=2)+'\n');t.replace(O)
save()
try:
 while True:
  p=json.loads(P.read_text())
  if p['status']!='running':break
  # This queue may wait only while the original owning producer and child exist.
  os.kill(p['producer_pid'],0);os.kill(p['child_pid'],0)
  time.sleep(10)
 d.update(status='running_snapshot_v4_gate',prior_root_result=p['status'],prior_root_source_unchanged=p.get('source_unchanged'));save()
 command=[sys.executable,str(R/'scripts/run-dedup-root-guard-snapshot-v4-decode-20261014.py')]
 with (D/'dedup-root-guard-snapshot-v4-queue-20261014.log').open('x')as log:
  child=subprocess.Popen(command,cwd=R,stdout=log,stderr=subprocess.STDOUT);d['child_pid']=child.pid;save();rc=child.wait()
 assert rc==0,('snapshot producer',rc)
 d.update(status='auditing_snapshot_v4_inventory');save();subprocess.run([sys.executable,str(R/'scripts/audit-dedup-root-guard-snapshot-v4-decode-20261014.py')],cwd=R,check=True)
 d.update(status='completed_snapshot_v4_selected_gate');save()
except BaseException as e:
 d.update(status='stopped_on_failure',error=repr(e));save();raise
