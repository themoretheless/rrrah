"""Prove the regression rejects omitted SIGKILL; kills only its own fixture child."""
import importlib.util
from pathlib import Path
import signal
import unittest
from unittest.mock import patch
root = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location('timeout_tests', root/'scripts/test-viewer-suite-timeout.py')
tests = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tests)
original = tests.gate.os.killpg
def omit_kill(group, sig):
    if sig != signal.SIGKILL:
        return original(group, sig)
suite = unittest.TestSuite([tests.SuiteTimeoutTests('test_timeout_stops_child_tree_and_preserves_output')])
with patch.object(tests.gate.os, 'killpg', omit_kill):
    result = unittest.TextTestRunner(verbosity=2).run(suite)
assert len(result.failures) == 1 and not result.errors and not result.skipped
assert 'child still executing' in result.failures[0][1]
print('Mutation correctly rejected: without group SIGKILL, SIGTERM-resistant child remains executing.')
