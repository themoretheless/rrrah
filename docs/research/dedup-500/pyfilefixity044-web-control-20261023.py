"""Execute selected original functions with dependency seams, never start a server."""
import ast
import hashlib
import json
import os
import threading
from pathlib import Path
from types import SimpleNamespace
from weakref import WeakValueDictionary
ROOT = Path(__file__).resolve().parents[3]
REL = "pyFileFixity/lib/profilers/visual/pympler/web.py"
raw = (ROOT / ".dedup-work/pyfilefixity044-20261023" / REL).read_bytes()
entries = json.loads((ROOT / "docs/research/dedup-500/evidence/lrq3000--pyFileFixity/tree.json").read_text())["tree"]
blob = hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest()
assert blob == next(x["sha"] for x in entries if x["path"] == REL)
module = ast.parse(raw)
names = {"ServerState", "get_ref", "get_obj", "get_obj_referents", "_get_graph"}
selected = []
for n in module.body:
    if isinstance(n, (ast.ClassDef, ast.FunctionDef)) and n.name in names:
        n.decorator_list = []
        selected.append(n)
ns = {"threading": threading, "WeakValueDictionary": WeakValueDictionary, "os": os}
exec(compile(ast.Module(body=selected, type_ignores=[]), REL, "exec"), ns)
server = ns["server"] = ns["ServerState"]()
class Falsey:
    def __bool__(self): return False
obj = Falsey()
ref = ns["get_ref"](obj)
try:
    ns["get_obj"](ref)
except KeyError:
    falsey_lookup_failed = True
else:
    raise AssertionError("expected falsey weak reference lookup failure")
strong = []
strong_ref = ns["get_ref"](strong)
server.clear_cache()
assert server.id2obj[int(strong_ref)] is strong
root = object()
root_ref = ns["get_ref"](root)
children = [[], []]
ns["asizeof"] = SimpleNamespace(_getreferents=lambda _: children, asizeof=lambda _: 1)
ns["safe_repr"] = lambda obj, clip: repr(obj)
referents = ns["get_obj_referents"](root_ref)["referents"]
assert len(referents) == 1
class Graph:
    def render(self, *args, **kwargs): return False
server.tmpdir = "/unused-no-files-created"
graph = Graph()
rendered = ns["_get_graph"](graph, "missing.png")
assert rendered == "missing.png"
print(json.dumps({"source_git_blob_verified": blob, "source_sha256": hashlib.sha256(raw).hexdigest(), "scope": "Exact selected AST bodies, decorators stripped; object and rendering seams only; no imports of upstream web framework, network listener, filesystem write or renderer", "falsey_weak_lookup_raises_keyerror": falsey_lookup_failed, "strong_reference_retained_after_clear_cache": True, "same_type_referents": {"input_count": 2, "display_count": len(referents)}, "render_false_reported_as_filename": rendered}, indent=2))
