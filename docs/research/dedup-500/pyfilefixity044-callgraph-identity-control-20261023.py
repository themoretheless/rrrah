"""Exact AST tracer on synthetic frames; no import-time monkeypatch or real tracing."""
import ast
import hashlib
import json
from pathlib import Path
from types import SimpleNamespace
ROOT = Path(__file__).resolve().parents[3]
REL = "pyFileFixity/lib/profilers/visual/pycallgraph.py"
raw = (ROOT / ".dedup-work/pyfilefixity044-20261023" / REL).read_bytes()
entries = json.loads((ROOT / "docs/research/dedup-500/evidence/lrq3000--pyFileFixity/tree.json").read_text())["tree"]
blob = hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest()
assert blob == next(x["sha"] for x in entries if x["path"] == REL)
names = {"reset_trace", "tracer", "get_dot", "_frac_calculation"}
selected = ast.Module(body=[n for n in ast.parse(raw).body if isinstance(n, ast.FunctionDef) and n.name in names], type_ignores=[])
clock = iter([1.0, 2.0, 3.0, 4.0])
ns = {"settings": {"include_stdlib": True, "node_colour": lambda *args: "0 0 0", "edge_colour": lambda *args: "0 0 0", "node_attributes": {}}, "graph_attributes": {}, "inspect": SimpleNamespace(getmodule=lambda code: SimpleNamespace(__name__="authored", __file__="authored.py")), "time": SimpleNamespace(time=lambda: next(clock)), "trace_filter": None, "time_filter": lambda **kwargs: True}
exec(compile(selected, REL, "exec"), ns)
ns["reset_trace"]()
for filename, firstline in [("first.py", 10), ("second.py", 90)]:
    frame = SimpleNamespace(f_code=SimpleNamespace(co_name="work", co_filename=filename, co_firstlineno=firstline), f_locals={})
    ns["tracer"](frame, "call", None)
    ns["tracer"](frame, "return", None)
assert ns["func_count"] == {"authored.work": 2}
stops = []
ns["stop_trace"] = lambda: stops.append(True)
ns["get_dot"](stop=True)
assert not stops
print(json.dumps({"source_git_blob_verified": blob, "source_sha256": hashlib.sha256(raw).hexdigest(), "scope": "Exact AST tracer and DOT helper; synthetic frames/module resolution/clock, no sys tracing, filesystem, renderer or monkeypatch of inspect", "distinct_code_locations": 2, "reported_function_nodes": ns["func_count"], "get_dot_stop_true_calls_stop": len(stops)}, indent=2))
