#!/usr/bin/env python3
"""Original pure flag filter and bounded synthetic multi-volume registry queries."""
from pathlib import Path
import hashlib
import json
import sys
import threading

checkout, output = [Path(p).resolve() for p in sys.argv[1:]]
sys.path.insert(0, str(checkout))
from copyparty.up2k import Up2k

source = checkout / "copyparty/up2k.py"
b = source.read_bytes()
tree = json.loads((Path(__file__).parent / "evidence/9001--copyparty/tree.json").read_text())
pin = next(x["sha"] for x in tree["tree"] if x["path"] == "copyparty/up2k.py")
assert hashlib.sha1(b"blob " + str(len(b)).encode() + b"\0" + b).hexdigest() == pin
obj = Up2k.__new__(Up2k)
flags = {"e2t": True, "e2ts": True, "e2d": True}
obj.no_expr_idx = False
changed, supported = obj._expr_idx_filter(flags)
assert not changed and supported == flags and supported is not flags
obj.no_expr_idx = True
changed, legacy = obj._expr_idx_filter(flags)
assert changed and legacy is flags and "e2t" in legacy and "e2ts" in legacy

def jobs(volume, count):
    return {str(n): {"t0": n, "vtop": volume, "prel": "", "name": str(n),
                    "size": 3, "user": "owner", "addr": "owned-fixture",
                    "need": ["a"], "hash": ["a", "b"]} for n in range(count)}
obj.reg_mutex = threading.Lock()
obj.flags = {"v0": {"u2abort": 1}, "v1": {"u2abort": 1}}
obj.registry = {"v0": jobs("v0", 1007)}
single = obj.get_unfinished_by_user("owner", "owned-fixture")
assert len(single["f"]) == 1000
obj.registry = {"v0": jobs("v0", 1000), "v1": jobs("v1", 7)}
multi = obj.get_unfinished_by_user("owner", "owned-fixture")
assert len(multi["f"]) == 1007
result = {"source_sha256": hashlib.sha256(b).hexdigest(), "controls": 4,
          "modern_filter_copies_flags": True,
          "legacy_filter_reports_change_but_keeps_e2t_flags": changed and legacy == flags,
          "legacy_returns_original_flags_object": legacy is flags,
          "single_volume_returned": len(single["f"]),
          "two_volume_returned": len(multi["f"]),
          "scope": "Complete original helper/query bodies, constructor-free object with synthetic owned metadata and actual lock. No SQLite<3.9, background indexer, live server, filesystem files or real user/session behavior."}
assert not output.exists()
output.write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps(result, indent=2))
