"""Original Request.body getter, in-memory streams only; no HTTP/server."""
import ast
import hashlib
import io
import json
from pathlib import Path
from types import SimpleNamespace
ROOT = Path(__file__).resolve().parents[3]
REL = "pyFileFixity/lib/profilers/visual/pympler/util/bottle3.py"
raw = (ROOT / ".dedup-work/pyfilefixity044-20261023" / REL).read_bytes()
entries = json.loads((ROOT / "docs/research/dedup-500/evidence/lrq3000--pyFileFixity/tree.json").read_text())["tree"]
blob = hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest()
assert blob == next(x["sha"] for x in entries if x["path"] == REL)
cls = next(n for n in ast.parse(raw).body if isinstance(n, ast.ClassDef) and n.name == "Request")
method = next(n for n in cls.body if isinstance(n, ast.FunctionDef) and n.name == "body")
method.decorator_list = []
ns = {"BytesIO": io.BytesIO, "MEMFILE_MAX": 1024}
exec(compile(ast.Module(body=[method], type_ignores=[]), REL, "exec"), ns)
short = SimpleNamespace(_body=None, content_length=5, environ={"wsgi.input": io.BytesIO(b"ab")})
actual = ns["body"](short).read()
assert actual == b"ab"
unknown = SimpleNamespace(_body=None, content_length=-1, environ={"wsgi.input": io.BytesIO(b"abc")})
assert ns["body"](unknown).read() == b""
print(json.dumps({"source_git_blob_verified": blob, "source_sha256": hashlib.sha256(raw).hexdigest(), "scope": "Exact AST getter with property decorator stripped, BytesIO and authored lengths; no HTTP/listener/files", "declared_length": 5, "returned_short_body_length": len(actual), "short_body_rejected": False, "unknown_length_returned_bytes": 0}, indent=2))
