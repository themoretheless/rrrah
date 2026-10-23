"""Original AST report and sampler controls with in-memory dependencies."""
import ast
import copy
import hashlib
import inspect
import io
import json
from pathlib import Path
from types import SimpleNamespace
ROOT = Path(__file__).resolve().parents[3]
REL = "pyFileFixity/lib/profilers/visual/memory_profiler.py"
raw = (ROOT / ".dedup-work/pyfilefixity044-20261023" / REL).read_bytes()
entries = json.loads((ROOT / "docs/research/dedup-500/evidence/lrq3000--pyFileFixity/tree.json").read_text())["tree"]
blob = hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest()
assert blob == next(x["sha"] for x in entries if x["path"] == REL)
selected = ast.Module(body=[n for n in ast.parse(raw).body if isinstance(n, ast.FunctionDef) and n.name in {"show_results", "memory_usage"}], type_ignores=[])
ns = {"os": SimpleNamespace(path=SimpleNamespace(exists=lambda _: True)), "inspect": inspect, "linecache": SimpleNamespace(getlines=lambda _: ["def authored():\n", "    x = 1\n", "    return x\n"])}
exec(compile(selected, REL, "exec"), ns)
class Code:
    co_filename = "authored.py"
    co_firstlineno = 1
code = Code()
lines = {2: [10.0, 20.0], 3: [30.0, 40.0, 50.0]}
before = copy.deepcopy(lines)
ns["show_results"](SimpleNamespace(code_map={code: lines}), stream=io.StringIO())
assert lines != before and lines[2] == [10.0, -1.0] and lines[3] == [30.0, 40.0, -1.0]
try: ns["memory_usage"](proc=123, timeout=.2, interval=.1)
except TypeError as error: timeout_error = str(error)
else: raise AssertionError("expected float range TypeError")
print(json.dumps({"source_git_blob_verified": blob, "source_sha256": hashlib.sha256(raw).hexdigest(), "scope": "Exact AST functions, source text and file existence seams; no actual files, sampling, process or tracing", "report_samples_before": before, "report_samples_after": lines, "numeric_pid_timeout_error": timeout_error}, indent=2))
