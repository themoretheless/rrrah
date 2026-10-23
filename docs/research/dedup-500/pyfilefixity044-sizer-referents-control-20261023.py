"""Exact AST referent and length helpers, no full sizer import or mutation."""
import ast
import hashlib
import json
from pathlib import Path
ROOT = Path(__file__).resolve().parents[3]
REL = "pyFileFixity/lib/profilers/visual/pympler/asizeof.py"
raw = (ROOT / ".dedup-work/pyfilefixity044-20261023" / REL).read_bytes()
entries = json.loads((ROOT / "docs/research/dedup-500/evidence/lrq3000--pyFileFixity/tree.json").read_text())["tree"]
blob = hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest()
assert blob == next(x["sha"] for x in entries if x["path"] == REL)
names = {"_func_refs", "_refs", "_dir2", "_len_slice"}
selected = ast.Module(body=[n for n in ast.parse(raw).body if isinstance(n, ast.FunctionDef) and n.name in names], type_ignores=[])
ns = {}
exec(compile(selected, REL, "exec"), ns)
def factory():
    payload = [1, 2, 3]
    def closure(): return payload
    return closure
fn = factory()
refs = list(ns["_func_refs"](fn, False))
assert all(value is not fn.__closure__ for value in refs)
assert all(value is not fn.__defaults__ for value in refs if value is not None)
forward = ns["_len_slice"](slice(0, 5, 1))
assert forward == 6 and len(range(0, 5, 1)) == 5
try: ns["_len_slice"](slice(0, 5, 0))
except ZeroDivisionError: zero_step_error = True
else: raise AssertionError("expected zero-step failure")
print(json.dumps({"source_git_blob_verified": blob, "source_sha256": hashlib.sha256(raw).hexdigest(), "scope": "Exact AST helpers only; authored closure and slice values; no full sizing or import", "closure_tuple_omitted_from_direct_referents": True, "slice_0_5_1_reported_length": forward, "range_0_5_1_actual_length": 5, "zero_step_slice_raises_zerodivisionerror": zero_step_error}, indent=2))
