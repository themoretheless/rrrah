"""Focused exact-AST controls with a deterministic get_referrers seam; no GUI."""
import ast
import hashlib
import inspect
import io
import json
from pathlib import Path
from types import SimpleNamespace

ROOT = Path(__file__).resolve().parents[3]
REL = "pyFileFixity/lib/profilers/visual/pympler/refbrowser.py"
raw = (ROOT / ".dedup-work/pyfilefixity044-20261023" / REL).read_bytes()
entries = json.loads((ROOT / "docs/research/dedup-500/evidence/lrq3000--pyFileFixity/tree.json").read_text())["tree"]
blob = hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest()
assert blob == next(x["sha"] for x in entries if x["path"] == REL)
selected = ast.Module(body=[n for n in ast.parse(raw).body if isinstance(n, ast.ClassDef) and n.name in {"_Node", "RefBrowser", "StreamBrowser"}], type_ignores=[])
refs = []
namespace = {"inspect": inspect, "summary": SimpleNamespace(_repr=str), "gc": SimpleNamespace(get_referrers=lambda _: refs)}
exec(compile(selected, REL, "exec"), namespace)
Browser, Stream = namespace["RefBrowser"], namespace["StreamBrowser"]
root = object()
legitimate = {"o": root, "children": [], "str_func": None}
refs[:] = [legitimate]
b = Browser(root, maxdepth=1)
tree = b.get_tree()
assert not tree.children
class EqualityProbe:
    def __eq__(self, other):
        raise RuntimeError("referrer equality invoked")
refs[:] = [EqualityProbe()]
try:
    Browser(root, maxdepth=1).get_tree()
except RuntimeError as error:
    equality_error = str(error)
else:
    raise AssertionError("expected equality to be invoked")
refs[:] = []
b = Browser(root, maxdepth=0)
b.get_tree()
first = len(b.ignore)
b.get_tree()
second = len(b.ignore)
assert second > first
output = io.StringIO()
Stream("ROOT", maxdepth=3, stream=output).print_tree()
assert output.getvalue() == "ROOT\n"
print(json.dumps({"source_git_blob_verified": blob, "source_sha256": hashlib.sha256(raw).hexdigest(), "scope": "Exact AST classes; get_referrers substituted, no full module import or Tk", "legitimate_dictionary_referrer_hidden": len(tree.children) == 0, "user_equality_exception": equality_error, "ignore_lengths_after_repeated_calls": [first, second], "default_print_tree_output": output.getvalue()}, indent=2))
