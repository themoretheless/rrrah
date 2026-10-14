#!/usr/bin/env python3
from pathlib import Path
import json,subprocess,sys,time,os
R=Path(__file__).resolve().parents[1];D=R/'docs/research';P=D/'dedup-root-guard-latch-decode-20261014.json';O=D/'dedup-root-guard-latch-audit-queue-20261014.json';assert not O.exists();d=dict(status='waiting_for_confirmed_live_root_gate',producer_pid=os.getpid())
def save():
 t=O.with_suffix('.tmp');t.write_text(json.dumps(d,indent=2)+'\n');t.replace(O)
save()
try:
 while True:
  p=json.loads(P.read_text())
  if p['status']!='running':break
  time.sleep(10)
 assert p['status']=='passed_pending_inventory_audit'and p['source_unchanged'],p
 d.update(status='auditing_current_root_inventory');save();subprocess.run([sys.executable,str(R/'scripts/audit-dedup-root-guard-latch-decode-20261014.py')],cwd=R,check=True);d.update(status='completed');save()
except BaseException as e:d.update(status='stopped_on_failure',error=repr(e));save();raise
