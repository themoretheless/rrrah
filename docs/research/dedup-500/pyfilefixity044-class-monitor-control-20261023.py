"""Exact tracker classes with synthetic snapshot callback; no background thread starts."""
import ast
import hashlib
import json
from pathlib import Path
from threading import Lock, Thread
from types import SimpleNamespace
ROOT = Path(__file__).resolve().parents[3]
REL = "pyFileFixity/lib/profilers/visual/pympler/classtracker.py"
raw = (ROOT / ".dedup-work/pyfilefixity044-20261023" / REL).read_bytes()
entries = json.loads((ROOT / "docs/research/dedup-500/evidence/lrq3000--pyFileFixity/tree.json").read_text())["tree"]
blob = hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest()
assert blob == next(x["sha"] for x in entries if x["path"] == REL)
selected = ast.Module(body=[n for n in ast.parse(raw).body if isinstance(n, ast.ClassDef) and n.name in {"PeriodicThread", "Snapshot", "ClassTracker"}], type_ignores=[])
ns = {"Thread": Thread, "Lock": Lock, "sleep": lambda interval: None}
exec(compile(selected, REL, "exec"), ns)
calls = []
def snapshot():
    calls.append(1)
    worker.stop = True
worker = ns["PeriodicThread"](SimpleNamespace(create_snapshot=snapshot), 1)
worker.stop = True
worker.run()
assert len(calls) == 1
tracker, other = ns["ClassTracker"](), ns["ClassTracker"]()
tracker._periodic_thread = worker
try: tracker.stop_periodic_snapshots()
except AttributeError as error: stop_error = str(error)
else: raise AssertionError("expected legacy isAlive interface failure")
s = ns["Snapshot"]()
s.system_total = SimpleNamespace(available=True, vsz=100, rss=20)
assert s.total == 100
print(json.dumps({"source_git_blob_verified": blob, "source_sha256": hashlib.sha256(raw).hexdigest(), "scope": "Exact AST classes, direct run with bounded callback and no-op sleep; actual Thread constructed but never started", "snapshots_despite_prior_stop": len(calls), "modern_thread_stop_error": stop_error, "snapshot_lock_shared_by_trackers": tracker.snapshot_lock is other.snapshot_lock, "snapshot_total_uses_vsz": s.total}, indent=2))
