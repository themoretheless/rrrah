"""Original typedef method with failing estimate and independent getsizeof seam."""
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
node = next(n for n in ast.parse(raw).body if isinstance(n, ast.ClassDef) and n.name == "_Typedef")
called = []
ns = {"_getsizeof": lambda obj, default: called.append(True) or 100}
exec(compile(ast.Module(body=[node], type_ignores=[]), REL, "exec"), ns)
def bad_length(obj): raise ValueError("authored estimate failure")
t = ns["_Typedef"].__new__(ns["_Typedef"])
t.base, t.item, t.leng = 1, 1, bad_length
try: t.flat(object())
except ValueError as error: observed = str(error)
else: raise AssertionError("expected failed estimate before size lookup")
assert called == []
t.leng = lambda obj: 4
assert t.flat(object()) == 100
print(json.dumps({"source_git_blob_verified": blob, "source_sha256": hashlib.sha256(raw).hexdigest(), "scope": "Exact _Typedef class, __new__ bypass of setup, length and getsizeof seams only; no full sizing module import", "estimate_error": observed, "getsizeof_called_after_failed_estimate": False, "getsizeof_overrides_successful_estimate": 100, "bytes_str_type_guard_value_on_runtime": type(bytes) is not type(str)}, indent=2))
