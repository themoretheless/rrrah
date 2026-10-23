"""Original template helper with inert adapters, plus isolated original import."""
import ast
import hashlib
import importlib.util
import json
import sys
from pathlib import Path
ROOT = Path(__file__).resolve().parents[3]
REL = "pyFileFixity/lib/profilers/visual/pympler/util/bottle3.py"
source = ROOT / ".dedup-work/pyfilefixity044-20261023" / REL
raw = source.read_bytes()
entries = json.loads((ROOT / "docs/research/dedup-500/evidence/lrq3000--pyFileFixity/tree.json").read_text())["tree"]
blob = hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest()
assert blob == next(x["sha"] for x in entries if x["path"] == REL)
class AdapterA:
    def __init__(self, **kwargs): pass
    def render(self, **kwargs): return "A"
class AdapterB(AdapterA):
    def render(self, **kwargs): return "B"
method = next(n for n in ast.parse(raw).body if isinstance(n, ast.FunctionDef) and n.name == "template")
ns = {"SimpleTemplate": AdapterA, "TEMPLATES": {}, "DEBUG": False, "TEMPLATE_PATH": [], "abort": lambda *args: None, "request": None, "response": None}
exec(compile(ast.Module(body=[method], type_ignores=[]), REL, "exec"), ns)
assert ns["template"]("authored", template_adapter=AdapterA) == "A"
second = ns["template"]("authored", template_adapter=AdapterB)
assert second == "A"
try:
    spec = importlib.util.spec_from_file_location("dedup_legacy_bottle3", source)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
except Exception as error:
    original_import = {"error": type(error).__name__, "message": str(error)}
else: original_import = {"result": "PASS"}
print(json.dumps({"source_git_blob_verified": blob, "source_sha256": hashlib.sha256(raw).hexdigest(), "scope": "Exact template AST with inert adapters; original import attempted in disposable interpreter, no network listener or request", "requested_second_adapter": "B", "actual_second_result": second, "python": sys.version.split()[0], "original_import": original_import}, indent=2))
