"""Exact report classes with authored snapshot data; no pickle, files or plots."""
import ast
import hashlib
import json
import os
from pathlib import Path
from types import SimpleNamespace
ROOT = Path(__file__).resolve().parents[3]
REL = "pyFileFixity/lib/profilers/visual/pympler/classtracker_stats.py"
raw = (ROOT / ".dedup-work/pyfilefixity044-20261023" / REL).read_bytes()
entries = json.loads((ROOT / "docs/research/dedup-500/evidence/lrq3000--pyFileFixity/tree.json").read_text())["tree"]
blob = hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest()
assert blob == next(x["sha"] for x in entries if x["path"] == REL)
selected = ast.Module(body=[n for n in ast.parse(raw).body if isinstance(n, ast.ClassDef) and n.name in {"Stats", "HtmlStats"}], type_ignores=[])
ns = {"os": os}
exec(compile(selected, REL, "exec"), ns)
observed = []
obj = SimpleNamespace(get_max_size=lambda: 100, get_size_at_time=lambda ts: observed.append(ts) or {1: 100, 2: 20}[ts])
tracker = SimpleNamespace(index={"Authored": [obj]}, snapshots=[SimpleNamespace(timestamp=1, tracked_total=100), SimpleNamespace(timestamp=2, tracked_total=20)])
stats = ns["Stats"](tracker=tracker, stream=object())
stats._init_sort()
assert observed == [2] and obj.tsize == 20
html = ns["HtmlStats"](stream=object())
html.basedir = "/data/report"
sibling = html.relative_path("/data/report-other/a")
assert sibling == "-other/a"
try: html.relative_path("/unrelated/a")
except UnboundLocalError: unrelated_error = True
else: raise AssertionError("expected unset relative path")
print(json.dumps({"source_git_blob_verified": blob, "source_sha256": hashlib.sha256(raw).hexdigest(), "scope": "Exact AST classes and authored snapshots, no sizing, pickle, output or charts", "expected_peak_timestamp": 1, "actual_peak_timestamp": observed[0], "expected_peak_object_size": 100, "actual_peak_object_size": obj.tsize, "sibling_prefix_relative_path": sibling, "unrelated_path_raises_unboundlocalerror": unrelated_error}, indent=2))
