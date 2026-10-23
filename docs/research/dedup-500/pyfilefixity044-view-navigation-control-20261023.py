"""Exact method excerpts with UI seams, no wx window or file access."""
import ast
import hashlib
import json
import textwrap
from pathlib import Path
from types import SimpleNamespace
ROOT = Path(__file__).resolve().parents[3]
REL = "pyFileFixity/lib/profilers/visual/runsnakerun/runsnake.py"
raw = (ROOT / ".dedup-work/pyfilefixity044-20261023" / REL).read_bytes()
entries = json.loads((ROOT / "docs/research/dedup-500/evidence/lrq3000--pyFileFixity/tree.json").read_text())["tree"]
blob = hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest()
assert blob == next(x["sha"] for x in entries if x["path"] == REL)
text = raw.decode()
methods = []
for start, end in [("    def OnUpView(", "    def OnBackView("), ("    def OnShallowerView(", "    def OnDeeperView(")]:
    methods.extend(ast.parse(textwrap.dedent(start + text.split(start, 1)[1].split(end, 1)[0])).body)
ns = {"_": lambda s: s}
exec(compile(ast.Module(body=methods, type_ignores=[]), REL, "exec"), ns)
activations, status = [], []
view = SimpleNamespace(activated_node=object(), adapter=SimpleNamespace(best_parent=lambda node: object(), label=lambda node: "authored"), OnNodeActivated=lambda event: activations.append(event), SetStatusText=status.append)
ns["OnUpView"](view, None)
assert not activations and status == ["No parents for the currently selected node: authored"]
square = SimpleNamespace(max_depth=None, max_depth_seen=5, Refresh=lambda: None)
ns["OnShallowerView"](SimpleNamespace(squareMap=square), None)
assert square.max_depth == 5
print(json.dumps({"source_git_blob_verified": blob, "source_sha256": hashlib.sha256(raw).hexdigest(), "scope": "Exact two method excerpts; synthetic adapter/UI callbacks, no GUI", "best_parent_available_but_activation_count": len(activations), "reported_status": status[0], "shallower_from_seen_depth5_actual_limit": square.max_depth}, indent=2))
