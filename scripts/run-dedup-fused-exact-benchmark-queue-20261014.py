#!/usr/bin/env python3
from pathlib import Path
import os,json,time,subprocess,sys,hashlib
R=Path(__file__).resolve().parents[1];D=R/'docs/research';G=D/'dedup-fused-exact-pair-controls-v2-20261014.json';O=D/'dedup-fused-exact-benchmark-queue-20261014.json';assert not O.exists();sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();B=R/'scripts/benchmark-dedup-fused-exact-pair-20261014.py';d=dict(status='waiting_for_confirmed_live_candidate_gate',producer_pid=os.getpid(),benchmark_producer_sha256=sha(B))
def save():
 t=O.with_suffix('.tmp');t.write_text(json.dumps(d,indent=2)+'\n');t.replace(O)
save()
try:
 while True:
  g=json.loads(G.read_text())
  if g['status']=='passed_pending_performance_and_independent_audit':break
  assert g['status']in ['building','build_succeeded','inventory','tests'],g
  time.sleep(10)
 assert g['failed']==0 and sha(B)==d['benchmark_producer_sha256'];d.update(status='benchmarking',gate_sha256=sha(G));save();subprocess.run([sys.executable,str(B)],cwd=R,check=True);d.update(status='completed_pending_independent_benchmark_audit');save()
except BaseException as e:d.update(status='stopped_on_failure',error=repr(e));save();raise
