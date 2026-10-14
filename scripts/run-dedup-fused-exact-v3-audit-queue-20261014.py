#!/usr/bin/env python3
from pathlib import Path
import json,time,sys,subprocess,os
R=Path(__file__).resolve().parents[1];D=R/'docs/research';Q=D/'dedup-fused-exact-benchmark-queue-v3-20261014.json';O=D/'dedup-fused-exact-v3-audit-queue-20261014.json';assert not O.exists();d=dict(status='waiting_for_confirmed_running_benchmark_queue',producer_pid=os.getpid())
def save():
 t=O.with_suffix('.tmp');t.write_text(json.dumps(d,indent=2)+'\n');t.replace(O)
save()
try:
 while True:
  q=json.loads(Q.read_text())
  if q['status']=='completed_pending_independent_benchmark_audit':break
  assert q['status']in ['waiting_for_confirmed_live_candidate_gate','benchmarking'],q
  time.sleep(10)
 d.update(status='auditing');save();subprocess.run([sys.executable,str(R/'scripts/audit-dedup-fused-exact-benchmark-v3-20261014.py')],cwd=R,check=True);d.update(status='completed');save()
except BaseException as e:d.update(status='stopped_on_failure',error=repr(e));save();raise
