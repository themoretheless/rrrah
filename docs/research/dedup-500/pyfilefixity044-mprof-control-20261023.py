"""Exact AST helpers, in-memory authored records, no plots/processes/deletions."""
import ast
import hashlib
import io
import json
import math
from pathlib import Path
from types import SimpleNamespace
ROOT = Path(__file__).resolve().parents[3]
REL = "pyFileFixity/lib/profilers/memory_profiler/mprof"
raw = (ROOT / ".dedup-work/pyfilefixity044-20261023" / REL).read_bytes()
entries = json.loads((ROOT / "docs/research/dedup-500/evidence/lrq3000--pyFileFixity/tree.json").read_text())["tree"]
blob = hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest()
assert blob == next(x["sha"] for x in entries if x["path"] == REL)
selected = ast.Module(body=[n for n in ast.parse(raw).body if isinstance(n, ast.FunctionDef) and n.name in {"get_profile_filenames", "read_mprofile_file"}], type_ignores=[])
ns = {"glob": SimpleNamespace(glob=lambda _: ["mprofile_20261023000000.dat"]), "osp": SimpleNamespace(isfile=lambda _: False, isdir=lambda _: False), "copy": SimpleNamespace(copy=lambda x: x[:])}
exec(compile(selected, REL, "exec"), ns)
dynamic_all = "".join(["a", "l", "l"])
try: ns["get_profile_filenames"](dynamic_all)
except ValueError as error: selection_error = str(error)
else: raise AssertionError("expected non-interned all failure")
ns["open"] = lambda *args: io.StringIO("MEM nan 1\nMEM inf 2\n")
parsed = ns["read_mprofile_file"]("authored-in-memory")
assert all(not math.isfinite(x) for x in parsed["mem_usage"])
ns["open"] = lambda *args: io.StringIO("MEM 1  2\n")
try: ns["read_mprofile_file"]("authored-double-space")
except ValueError: double_space_failed = True
else: raise AssertionError("expected double-space record failure")
print(json.dumps({"source_git_blob_verified": blob, "source_sha256": hashlib.sha256(raw).hexdigest(), "scope": "Exact AST helpers; synthetic glob and StringIO open seams", "equal_dynamic_all_selection_error": selection_error, "nonfinite_memory_samples_accepted": len(parsed["mem_usage"]), "double_space_record_failed": double_space_failed}, indent=2))
