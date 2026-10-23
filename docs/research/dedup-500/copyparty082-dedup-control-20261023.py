#!/usr/bin/env python3
"""Original dedup/copy/move tests with owned tempfiles instead of global RAM-disk helper."""
from pathlib import Path
import hashlib
import importlib
import json
import os
import sys
import tempfile
import unittest

checkout, work = [Path(p).resolve() for p in sys.argv[1:3]]
modules = sys.argv[3:] or ["tests.test_dedup"]
correct_no_dedup = "--correct-no-dedup-fixture" in modules
if correct_no_dedup:
    modules.remove("--correct-no-dedup-fixture")
assert set(modules) <= {"tests.test_dedup", "tests.test_cp", "tests.test_mv"}
work.mkdir(exist_ok=False)
config = work / "config"
config.mkdir()
os.environ["XDG_CONFIG_HOME"] = str(config)
sys.path.insert(0, str(checkout))
tree = json.loads((Path(__file__).parent / "evidence/9001--copyparty/tree.json").read_text())
pins = {x["path"]: x["sha"] for x in tree["tree"] if x["type"] == "blob"}
for name in [m.replace(".", "/") + ".py" for m in modules] + ["tests/util.py", "tests/__init__.py"]:
    b = (checkout / name).read_bytes()
    assert hashlib.sha1(b"blob " + str(len(b)).encode() + b"\0" + b).hexdigest() == pins[name]

from tests import util as tu

# Original helper sweeps shared cptd-* directories and may create a macOS RAM disk.
# Restrict only fixture allocation. Do not invoke that helper or open a network server.
tu.get_ramdisk = lambda: tempfile.mkdtemp(prefix="owned-dedup-", dir=work)
corrected_calls = []
if correct_no_dedup:
    def corrected_cfg(*args, **kwargs):
        if kwargs.pop("no_dedup", False):
            kwargs["dedup"] = False
            corrected_calls.append(True)
        return tu.Cfg(*args, **kwargs)
counts = {}
def counted(name, original):
    def call(self, *args, **kwargs):
        counts[name] += 1
        return original(self, *args, **kwargs)
    return call

suite = unittest.TestSuite()
for module_name in modules:
    module = importlib.import_module(module_name)
    if correct_no_dedup:
        # Keep tests.util.Cfg itself unchanged: its super(Cfg, self) resolves that global.
        module.Cfg = corrected_cfg
    for cls in vars(module).values():
        if not isinstance(cls, type) or not issubclass(cls, unittest.TestCase) or cls.__module__ != module_name:
            continue
        for name in ["do_tc", "handshake", "put_chunk", "curl", "do_cp", "do_mv"]:
            if hasattr(cls, name):
                key = module_name + "." + name
                counts[key] = 0
                setattr(cls, name, counted(key, getattr(cls, name)))
    suite.addTests(unittest.defaultTestLoader.loadTestsFromModule(module))
result = unittest.TextTestRunner(verbosity=2).run(suite)
summary = {"tests_run": result.testsRun, "failures": len(result.failures),
           "errors": len(result.errors), "skipped": len(result.skipped),
           "method_calls": counts, "modules": modules, "python": sys.version,
           "fixture_no_dedup_translation": correct_no_dedup,
           "corrected_config_calls": len(corrected_calls),
           "scope": "Original whole selected dedup/copy/move test methods and actual filesystem/SQLite/up2k with original in-memory HTTP test fixtures. Owned tempfile allocator replaces global RAM-disk helper; counting wrappers delegate unchanged bodies. Not real socket server, production deployment or full suite qualification."}
(work / "result.json").write_text(json.dumps(summary, indent=2) + "\n")
print(json.dumps(summary))
raise SystemExit(0 if result.wasSuccessful() else 1)
