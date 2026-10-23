"""Original AST function controls with synthetic sampler and psutil seams."""
import ast
import hashlib
import json
import os
import subprocess
from pathlib import Path
from types import SimpleNamespace
ROOT = Path(__file__).resolve().parents[3]
REL = "pyFileFixity/lib/profilers/memory_profiler/memory_profiler.py"
raw = (ROOT / ".dedup-work/pyfilefixity044-20261023" / REL).read_bytes()
entries = json.loads((ROOT / "docs/research/dedup-500/evidence/lrq3000--pyFileFixity/tree.json").read_text())["tree"]
blob = hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest()
assert blob == next(x["sha"] for x in entries if x["path"] == REL)
module = ast.Module(body=[n for n in ast.parse(raw).body if isinstance(n, ast.FunctionDef) and n.name in {"_get_memory", "memory_usage"}], type_ignores=[])
class ModernProcess:
    def memory_info(self): return (1024,)
ns = {"os": os, "has_psutil": True, "psutil": SimpleNamespace(Process=lambda pid: ModernProcess(), AccessDenied=PermissionError), "_TWO_20": float(2**20), "subprocess": subprocess}
exec(compile(module, REL, "exec"), ns)
try: ns["_get_memory"](1)
except AttributeError as error: psutil_error = str(error)
else: raise AssertionError("expected eager deprecated attribute evaluation")
timers = []
class Timer:
    def __init__(self, *args, **kwargs):
        self.joined = False
        timers.append(self)
    def start(self): pass
    def join(self, timeout): self.joined = True
class Connection:
    def __init__(self): self.position = 0
    def recv(self):
        self.position += 1
        return {1: 0, 2: [10.0], 3: (1 if len(timers) == 1 else 5)}[self.position]
    def send(self, value): pass
ns["Pipe"] = lambda: (object(), Connection())
ns["MemTimer"] = Timer
calls = []
def work():
    calls.append(len(calls))
    return len(calls)
result = ns["memory_usage"](work, retval=True)
assert len(calls) == 2 and result == ([10.0], 2)
def fail(): raise ValueError("authored failure")
try: ns["memory_usage"](fail)
except ValueError: pass
else: raise AssertionError("expected callable failure")
assert not timers[-1].joined
print(json.dumps({"source_git_blob_verified": blob, "source_sha256": hashlib.sha256(raw).hexdigest(), "scope": "Exact AST _get_memory and memory_usage; synthetic modern psutil process and timer/pipe only; no process, real sampler or sleep", "modern_psutil_interface_error": psutil_error, "single_request_callable_executions": len(calls), "returned_value_from_final_execution": result[1], "callable_exception_skips_join": not timers[-1].joined}, indent=2))
