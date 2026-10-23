"""Exact StatisticalThread class; construct but never start a thread."""
import ast
import hashlib
import json
import threading
import _thread
from pathlib import Path
from types import SimpleNamespace
ROOT = Path(__file__).resolve().parents[3]
REL = "pyFileFixity/lib/profilers/pprofile/pprofile.py"
raw = (ROOT / ".dedup-work/pyfilefixity044-20261023" / REL).read_bytes()
entries = json.loads((ROOT / "docs/research/dedup-500/evidence/lrq3000--pyFileFixity/tree.json").read_text())["tree"]
blob = hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest()
assert blob == next(x["sha"] for x in entries if x["path"] == REL)
source = raw.decode().split("class StatisticalThread(", 1)[1].split("# profile/cProfile-like API (no sort parameter !)", 1)[0]
ns = {"threading": threading, "thread": _thread, "ProfileRunnerBase": object, "time": lambda: 11.0}
exec(compile(ast.parse("class StatisticalThread(" + source), REL, "exec"), ns)
prof = SimpleNamespace(total_time=0)
worker = ns["StatisticalThread"](prof)
worker.is_alive = lambda: True
worker._start_time = 10.0
worker.stop()
assert prof.total_time == 1.0
try: worker.stop()
except TypeError as error: second_stop_error = str(error)
else: raise AssertionError("expected stop race failure")
print(json.dumps({"source_git_blob_verified": blob, "source_sha256": hashlib.sha256(raw).hexdigest(), "scope": "Exact StatisticalThread class, constructed not started; is_alive seam keeps stopping thread live, deterministic clock", "first_stop_elapsed": prof.total_time, "second_stop_error": second_stop_error}, indent=2))
