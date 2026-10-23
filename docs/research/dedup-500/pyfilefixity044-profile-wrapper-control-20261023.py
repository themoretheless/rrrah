"""Original AST wrapper classes with authored profiler/clock seams; no hooks."""
import ast
import hashlib
import json
from pathlib import Path
from types import SimpleNamespace
ROOT = Path(__file__).resolve().parents[3]
REL = "pyFileFixity/lib/profilers/visual/profilehooks.py"
raw = (ROOT / ".dedup-work/pyfilefixity044-20261023" / REL).read_bytes()
entries = json.loads((ROOT / "docs/research/dedup-500/evidence/lrq3000--pyFileFixity/tree.json").read_text())["tree"]
blob = hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest()
assert blob == next(x["sha"] for x in entries if x["path"] == REL)
selected = ast.Module(body=[n for n in ast.parse(raw).body if isinstance(n, ast.ClassDef) and n.name in {"FuncProfile", "FuncTimer"}], type_ignores=[])
ns = {"Profile": object, "atexit": SimpleNamespace(register=lambda fn: None)}
exec(compile(selected, REL, "exec"), ns)
class Backend:
    def runcall(self, fn, *args, **kwargs): return fn(*args, **kwargs)
def target(): raise ValueError("target failure")
def report(backend): raise RuntimeError("report failure")
profile = ns["FuncProfile"].__new__(ns["FuncProfile"])
profile.ncalls, profile.skip, profile.immediate = 0, 0, False
profile.Profile, profile.fn = Backend, target
profile.stats = SimpleNamespace(add=report)
try: profile()
except RuntimeError as error:
    observed = {"raised": str(error), "original_context": type(error.__context__).__name__}
else: raise AssertionError("expected report masking target failure")
assert ns["FuncProfile"].in_profiler is False
clock = iter([2.0, 1.0])
timer = ns["FuncTimer"](lambda: 42, False, lambda: next(clock))
assert timer() == 42 and timer.totaltime == -1.0
print(json.dumps({"source_git_blob_verified": blob, "source_sha256": hashlib.sha256(raw).hexdigest(), "scope": "Exact AST classes with authored backend/report and backward clock; atexit registration replaced, no actual profiler or timer", "exception_masking": observed, "shared_flag_cleared_after_failure": True, "return_value_preserved": 42, "accepted_negative_duration": timer.totaltime}, indent=2))
