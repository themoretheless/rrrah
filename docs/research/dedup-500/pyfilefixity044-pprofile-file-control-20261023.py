"""Exact parsed prefix classes; no Python2 port or trace hook."""
import ast
import hashlib
import json
from pathlib import Path
from types import SimpleNamespace
ROOT = Path(__file__).resolve().parents[3]
REL = "pyFileFixity/lib/profilers/pprofile/pprofile.py"
raw = (ROOT / ".dedup-work/pyfilefixity044-20261023" / REL).read_bytes()
entries = json.loads((ROOT / "docs/research/dedup-500/evidence/lrq3000--pyFileFixity/tree.json").read_text())["tree"]
blob = hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest()
assert blob == next(x["sha"] for x in entries if x["path"] == REL)
prefix = raw.decode().split("\n    def callgrind(", 1)[0]
selected = ast.Module(body=[n for n in ast.parse(prefix).body if isinstance(n, ast.ClassDef) and n.name in {"_FileTiming", "ProfileBase"}], type_ignores=[])
ns = {}
exec(compile(selected, REL, "exec"), ns)
prof = ns["ProfileBase"]()
shared_globals = {}
a = prof._getFileTiming(SimpleNamespace(f_globals=shared_globals, f_code=SimpleNamespace(co_filename="first.py")))
b = prof._getFileTiming(SimpleNamespace(f_globals=shared_globals, f_code=SimpleNamespace(co_filename="second.py")))
assert a is b and b.filename == "first.py"
print(json.dumps({"source_git_blob_verified": blob, "source_sha256": hashlib.sha256(raw).hexdigest(), "scope": "Exact prefix AST classes, ProfileBase ends before callgrind; synthetic frames with shared globals and different code filenames", "second_file_expected": "second.py", "second_file_actual": b.filename, "distinct_source_files_merged": a is b}, indent=2))
