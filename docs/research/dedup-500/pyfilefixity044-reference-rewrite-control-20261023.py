"""Exact helper excerpts; no Python2 module import, dump loading or GUI."""
import ast
import hashlib
import json
from pathlib import Path
ROOT = Path(__file__).resolve().parents[3]
REL = "pyFileFixity/lib/profilers/visual/runsnakerun/meliaeloader.py"
raw = (ROOT / ".dedup-work/pyfilefixity044-20261023" / REL).read_bytes()
entries = json.loads((ROOT / "docs/research/dedup-500/evidence/lrq3000--pyFileFixity/tree.json").read_text())["tree"]
blob = hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest()
assert blob == next(x["sha"] for x in entries if x["path"] == REL)
text = raw.decode()
helpers = []
for start, end in [("def as_id(", "def rewrite_refs("), ("def rewrite_references(", "def simple(")]:
    excerpt = start + text.split(start, 1)[1].split(end, 1)[0]
    helpers.extend(ast.parse(excerpt).body)
ns = {}
exec(compile(ast.Module(body=helpers, type_ignores=[]), REL, "exec"), ns)
a = ns["rewrite_references"]([1, 2], 1, 2, single_ref=True)
b = ns["rewrite_references"]([2, 1], 1, 2, single_ref=True)
assert a == [2, 2] and b == [2]
print(json.dumps({"source_git_blob_verified": blob, "source_sha256": hashlib.sha256(raw).hexdigest(), "scope": "Exact as_id and rewrite_references excerpts; authored integer reference lists only", "old_before_existing_new": a, "existing_new_before_old": b, "single_ref_result_depends_on_input_order": True}, indent=2))
