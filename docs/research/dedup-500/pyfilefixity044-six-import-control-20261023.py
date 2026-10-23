"""Import original six in this disposable interpreter; no installed package changes."""
import hashlib
import importlib
import importlib.util
import json
import sys
from pathlib import Path
ROOT = Path(__file__).resolve().parents[3]
REL = "pyFileFixity/lib/six.py"
source = ROOT / ".dedup-work/pyfilefixity044-20261023" / REL
raw = source.read_bytes()
entries = json.loads((ROOT / "docs/research/dedup-500/evidence/lrq3000--pyFileFixity/tree.json").read_text())["tree"]
blob = hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest()
assert blob == next(x["sha"] for x in entries if x["path"] == REL)
name = "dedup_legacy_six"
spec = importlib.util.spec_from_file_location(name, source)
module = importlib.util.module_from_spec(spec)
sys.modules[name] = module
spec.loader.exec_module(module)
assert module.b("é") == b"\xe9"
assert list(module.iteritems({"key": 1})) == [("key", 1)]
assert list(module.moves.range(3)) == [0, 1, 2]
try:
    moved = importlib.import_module(name + ".moves")
except Exception as error:
    moved_import = {"result": type(error).__name__, "message": str(error)}
else:
    moved_import = {"result": "PASS", "same_moves_object": moved is module.moves}
print(json.dumps({"source_git_blob_verified": blob, "source_sha256": hashlib.sha256(raw).hexdigest(), "scope": "Original full module imported from verified file in disposable interpreter under unique name; no dependency installs or network", "python": sys.version.split()[0], "six_version": module.__version__, "base_import": "PASS", "latin1_byte_literal": "PASS", "iteritems": "PASS", "direct_moves_range": "PASS", "import_moves_submodule": moved_import}, indent=2))
