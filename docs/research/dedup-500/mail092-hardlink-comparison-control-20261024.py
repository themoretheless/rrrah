from pathlib import Path
from types import SimpleNamespace
import ast, filecmp, json, os, tempfile, sys
root = Path(__file__).resolve().parents[3]
evidence = root / "docs/research/dedup-500/evidence/kdeldycke--mail-deduplicate"
source = evidence / "07-action.py"
tree = ast.parse(source.read_text())
function = next(n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == "hardlink_blocker")
module = ast.Module(body=[ast.ImportFrom(module="__future__", names=[ast.alias(name="annotations")], level=0), function], type_ignores=[])
ns = {"os": os, "filecmp": filecmp, "has_own_file": lambda mail: True}
exec(compile(ast.fix_missing_locations(module), str(source), "exec"), ns)
owned = Path(tempfile.mkdtemp(prefix="mail092-readonly-comparison-", dir=root / ".dedup-work"))
a, b = owned / "a", owned / "b"
a.write_bytes(b"aaaa"); b.write_bytes(b"aaaa")
mail, target = SimpleNamespace(path=str(a)), SimpleNamespace(path=str(b))
conf = SimpleNamespace(conf={"hardlink_differing": False})
filecmp.clear_cache()
assert ns["hardlink_blocker"](conf, mail, target) is None
before = b.stat()
b.write_bytes(b"bbbb")
os.utime(b, ns=(before.st_atime_ns, before.st_mtime_ns))
assert a.read_bytes() != b.read_bytes()
stale = ns["hardlink_blocker"](conf, mail, target)
assert stale is None
filecmp.clear_cache()
fresh = ns["hardlink_blocker"](conf, mail, target)
assert isinstance(fresh, str) and "differs byte for byte" in fresh
result = {"python": sys.version, "scope": "Exact upstream function AST; folder-file admission stub always true; stdlib filecmp and os real; no hardlink/replacement/deletion called", "equal_control_allowed": True, "different_same_metadata_allowed_with_warm_cache": stale is None, "cache_clear_restores_rejection": fresh, "owned_directory": str(owned)}
(root / "docs/research/dedup-500/mail092-hardlink-comparison-result-20261024.json").write_text(json.dumps(result, indent=2)+"\n")
print(json.dumps(result))
