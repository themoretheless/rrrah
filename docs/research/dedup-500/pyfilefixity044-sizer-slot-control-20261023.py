"""Exact AST helpers only; original module import incompatibility unchanged."""
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
selected = ast.Module(body=[n for n in ast.parse(raw).body if isinstance(n, ast.FunctionDef) and n.name in {"_dir2", "_repr"}], type_ignores=[])
ns = {"_Slots": lambda fields: fields, "_items": lambda fields: iter(fields.items())}
exec(compile(selected, REL, "exec"), ns)
class OneSlot:
    __slots__ = "payload"
    def __init__(self): self.payload = [1, 2, 3]
obj = OneSlot()
refs = list(ns["_dir2"](obj, slots="__slots__"))
assert refs == [("__slots__", {})]
class BadRepr:
    def __repr__(self): raise ValueError("authored repr failure")
try: ns["_repr"](BadRepr())
except ValueError as error: repr_error = str(error)
else: raise AssertionError("expected repr failure")
print(json.dumps({"source_git_blob_verified": blob, "source_sha256": hashlib.sha256(raw).hexdigest(), "scope": "Exact AST helpers, _Slots and _items dependency seams; no full module import or sizing", "string_slot_payload_omitted": True, "returned_slot_entries": refs, "safe_repr_valueerror": repr_error}, indent=2))
