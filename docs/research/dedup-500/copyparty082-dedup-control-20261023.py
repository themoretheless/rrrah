#!/usr/bin/env python3
"""Original dedup test bodies with an owned tempfile instead of global RAM-disk helper."""
from pathlib import Path
import hashlib
import json
import os
import sys
import tempfile
import unittest

checkout, work = [Path(p).resolve() for p in sys.argv[1:]]
work.mkdir(exist_ok=False)
config = work / "config"
config.mkdir()
os.environ["XDG_CONFIG_HOME"] = str(config)
sys.path.insert(0, str(checkout))
tree = json.loads((Path(__file__).parent / "evidence/9001--copyparty/tree.json").read_text())
pins = {x["path"]: x["sha"] for x in tree["tree"] if x["type"] == "blob"}
for name in ["tests/test_dedup.py", "tests/util.py", "tests/__init__.py"]:
    b = (checkout / name).read_bytes()
    assert hashlib.sha1(b"blob " + str(len(b)).encode() + b"\0" + b).hexdigest() == pins[name]

from tests import util as tu

# Original helper sweeps shared cptd-* directories and may create a macOS RAM disk.
# Restrict only fixture allocation. Do not invoke that helper or open a network server.
tu.get_ramdisk = lambda: tempfile.mkdtemp(prefix="owned-dedup-", dir=work)
from tests import test_dedup as upstream

counts = {"do_tc": 0, "handshake": 0, "put_chunk": 0, "curl": 0}
def counted(name, original):
    def call(self, *args, **kwargs):
        counts[name] += 1
        return original(self, *args, **kwargs)
    return call

for name in counts:
    setattr(upstream.TestDedup, name, counted(name, getattr(upstream.TestDedup, name)))
suite = unittest.defaultTestLoader.loadTestsFromTestCase(upstream.TestDedup)
result = unittest.TextTestRunner(verbosity=2).run(suite)
summary = {"tests_run": result.testsRun, "failures": len(result.failures),
           "errors": len(result.errors), "skipped": len(result.skipped),
           "method_calls": counts, "python": sys.version,
           "scope": "Original whole TestDedup methods and actual filesystem/SQLite/up2k with original in-memory HTTP test fixtures. Owned tempfile allocator replaces global RAM-disk helper; counting wrappers delegate unchanged bodies. Not real socket server, production deployment or full suite qualification."}
(work / "result.json").write_text(json.dumps(summary, indent=2) + "\n")
print(json.dumps(summary))
raise SystemExit(0 if result.wasSuccessful() else 1)
