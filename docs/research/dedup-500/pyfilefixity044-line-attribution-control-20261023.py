"""Exact original LineProfiler callback with synthetic nested frames; no tracing hook."""
import ast
import hashlib
import json
from pathlib import Path
from types import SimpleNamespace
ROOT = Path(__file__).resolve().parents[3]
REL = "pyFileFixity/lib/profilers/memory_profiler/memory_profiler.py"
raw = (ROOT / ".dedup-work/pyfilefixity044-20261023" / REL).read_bytes()
entries = json.loads((ROOT / "docs/research/dedup-500/evidence/lrq3000--pyFileFixity/tree.json").read_text())["tree"]
blob = hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest()
assert blob == next(x["sha"] for x in entries if x["path"] == REL)
selected = ast.Module(body=[n for n in ast.parse(raw).body if isinstance(n, ast.ClassDef) and n.name == "LineProfiler"], type_ignores=[])
ns = {"_get_memory": lambda *args, **kwargs: 42.0}
exec(compile(selected, REL, "exec"), ns)
prof = ns["LineProfiler"]()
prof._original_trace_function = None
outer, inner = object(), object()
prof.code_map = {outer: {}, inner: {}}
for code, event, line in [(outer, "call", 10), (outer, "line", 11), (inner, "call", 20), (inner, "line", 21), (inner, "return", 22), (outer, "line", 12)]:
    prof.trace_memory_usage(SimpleNamespace(f_code=code, f_lineno=line), event, None)
assert 22 in prof.code_map[outer]
assert 11 not in prof.code_map[outer]
print(json.dumps({"source_git_blob_verified": blob, "source_sha256": hashlib.sha256(raw).hexdigest(), "scope": "Exact AST LineProfiler with synthetic event sequence and fixed memory sample, no sys.settrace or real RSS sampling", "outer_expected_measured_lines": [10, 11], "outer_actual_measured_lines": sorted(prof.code_map[outer]), "inner_actual_measured_lines": sorted(prof.code_map[inner]), "nested_return_line_misattributed_to_outer": True}, indent=2))
