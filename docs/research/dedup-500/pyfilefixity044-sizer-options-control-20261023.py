"""Exact Asizer.set option handling, no original module initialization."""
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
node = next(n for n in ast.parse(raw).body if isinstance(n, ast.ClassDef) and n.name == "Asizer")
ns = {"_kind_ignored": "ignored"}
exec(compile(ast.Module(body=[node], type_ignores=[]), REL, "exec"), ns)
a = ns["Asizer"].__new__(ns["Asizer"])
a.set(code=True)
a.set(code=False)
assert not a._code_ and a._incl == " (incl. code)"
try: a.set(align=3)
except ValueError: pass
else: raise AssertionError("expected rejected alignment")
assert a._align_ == 3 and a._mask == 2
print(json.dumps({"source_git_blob_verified": blob, "source_sha256": hashlib.sha256(raw).hexdigest(), "scope": "Exact Asizer class, __new__ bypass and original set only; no object sizing or module import", "code_disabled_but_label": a._incl, "rejected_alignment_left_state": {"align": a._align_, "mask": a._mask}}, indent=2))
