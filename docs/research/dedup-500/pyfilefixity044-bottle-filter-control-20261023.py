"""Exact Bottle.add_filter method with inert callbacks, no server or import."""
import ast
import hashlib
import json
from pathlib import Path
from types import SimpleNamespace
ROOT = Path(__file__).resolve().parents[3]
REL = "pyFileFixity/lib/profilers/visual/pympler/util/bottle3.py"
raw = (ROOT / ".dedup-work/pyfilefixity044-20261023" / REL).read_bytes()
entries = json.loads((ROOT / "docs/research/dedup-500/evidence/lrq3000--pyFileFixity/tree.json").read_text())["tree"]
blob = hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest()
assert blob == next(x["sha"] for x in entries if x["path"] == REL)
cls = next(n for n in ast.parse(raw).body if isinstance(n, ast.ClassDef) and n.name == "Bottle")
method = next(n for n in cls.body if isinstance(n, ast.FunctionDef) and n.name == "add_filter")
ns = {}
exec(compile(ast.Module(body=[method], type_ignores=[]), REL, "exec"), ns)
app = SimpleNamespace(castfilter=[])
ns["add_filter"](app, int, lambda value: value)
try: ns["add_filter"](app, str, lambda value: value)
except TypeError as error: observed = str(error)
else: raise AssertionError("expected type-object ordering failure")
assert len(app.castfilter) == 2
print(json.dumps({"source_git_blob_verified": blob, "source_sha256": hashlib.sha256(raw).hexdigest(), "scope": "Exact AST add_filter, inert authored callbacks, no Bottle import/listener/request", "second_filter_error": observed, "filters_retained_after_failure": len(app.castfilter)}, indent=2))
