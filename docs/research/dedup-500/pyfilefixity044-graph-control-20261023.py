"""Execute exact AST-extracted upstream classes; no imports, renderer or sizing backend."""
import ast
import gc
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
REL = "pyFileFixity/lib/profilers/visual/pympler/refgraph.py"
SOURCE = ROOT / ".dedup-work/pyfilefixity044-20261023" / REL
raw = SOURCE.read_bytes()
tree = json.loads((ROOT / "docs/research/dedup-500/evidence/lrq3000--pyFileFixity/tree.json").read_text())
entries = tree["tree"] if isinstance(tree, dict) else tree
expected = next(x["sha"] for x in entries if x["path"] == REL)
actual = hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest()
assert actual == expected
module = ast.parse(raw)
selected = ast.Module(body=[n for n in module.body if isinstance(n, ast.ClassDef) and n.name in {"_Edge", "_MetaObject", "ReferenceGraph"}], type_ignores=[])
namespace = {"get_referents": gc.get_referents}
exec(compile(selected, str(SOURCE), "exec"), namespace)
Edge, Graph, Meta = (namespace[n] for n in ("_Edge", "ReferenceGraph", "_MetaObject"))
a, b = Edge(-1, 0, ""), Edge(-2, 0, "")
assert a.src != b.src and hash(a) == hash(b) and a == b
cycle = []
cycle.append(cycle)
predecessor = [cycle]
g = Graph.__new__(Graph)
g.objects = [predecessor, cycle]
retained = g._reduce_to_cycles()
assert retained == 2 and g.objects[0] is predecessor
h = Graph.__new__(Graph)
h.metadata = []
for ident in range(4):
    item = Meta()
    item.id = ident
    h.metadata.append(item)
h.edges = {Edge(0, 3, ""), Edge(1, 2, ""), Edge(2, 3, "")}
h._annotate_groups()
groups = [x.group for x in h.metadata]
assert len(set(groups)) > 1
print(json.dumps({"source_git_blob_verified": actual, "source_sha256": hashlib.sha256(raw).hexdigest(), "scope": "Exact AST classes; synthetic edges and actual in-memory gc referents, no original imports or Graph constructor/Asizer/Graphviz", "edge_hash_collision": {"distinct_endpoints": True, "reported_equal": a == b, "note": "Synthetic integer IDs demonstrate equality contract; no claim of observed live object-id collision"}, "cycle_reduction": {"actual_cycle_nodes": 1, "retained_nodes": retained, "predecessor_incorrectly_retained": True}, "connected_component": {"undirected_connected": True, "reported_groups": groups}}, indent=2))
