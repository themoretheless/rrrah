"""Exact pure layout helper excerpts, no wx import or drawing."""
import ast
import hashlib
import json
from pathlib import Path
ROOT = Path(__file__).resolve().parents[3]
REL = "pyFileFixity/lib/profilers/visual/runsnakerun/squaremap/squaremap.py"
raw = (ROOT / ".dedup-work/pyfilefixity044-20261023" / REL).read_bytes()
entries = json.loads((ROOT / "docs/research/dedup-500/evidence/lrq3000--pyFileFixity/tree.json").read_text())["tree"]
blob = hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest()
assert blob == next(x["sha"] for x in entries if x["path"] == REL)
source = "def coord_bigger_than_padding(" + raw.decode().split("def coord_bigger_than_padding(", 1)[1].split("class DefaultAdapter(", 1)[0]
ns = {}
exec(compile(ast.parse(source), REL, "exec"), ns)
valid = ns["split_box"](.25, 0, 0, 100, 50)
assert valid == ((0, 0, 25, 50), (25, 0, 75, 50))
tiny = ns["split_box"](.001, 0, 0, 100, 50)
assert tiny == (None, None)
over = ns["split_box"] (1.2, 0, 0, 100, 50)
assert over[1][2] == -20
print(json.dumps({"source_git_blob_verified": blob, "source_sha256": hashlib.sha256(raw).hexdigest(), "scope": "Exact pure helper excerpts, authored geometry only; no original wx import or rendered UI", "valid_split": valid, "subpixel_split_hides_both": tiny, "out_of_range_fraction_negative_tail": over, "note": "Out-of-range input demonstrates missing validation, not proof a production adapter supplies it"}, indent=2))
