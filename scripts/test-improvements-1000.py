#!/usr/bin/env python3
"""Regression checks for registry execution metadata validation."""
import copy
import importlib.util
import json
import unittest
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('registry_validator', ROOT / 'scripts/validate-improvements-1000.py')
validator = importlib.util.module_from_spec(spec)
spec.loader.exec_module(validator)

class ExecutionValidation(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.data = json.loads((ROOT / 'docs/improvements-1000.json').read_text())
        cls.markdown = (ROOT / 'docs/IMPROVEMENTS_1000.md').read_text()

    def errors_for(self, mutate):
        data = copy.deepcopy(self.data)
        mutate(data['tasks'][0]['execution'])
        return validator.validate(data, self.markdown)[0]

    def test_invalid_status_is_refused(self):
        self.assertTrue(any('invalid execution status' in e for e in self.errors_for(lambda x: x.update(status='done-ish'))))

    def test_missing_result_is_refused(self):
        self.assertTrue(any('missing execution result' in e for e in self.errors_for(lambda x: x.update(result=''))))

    def test_unsafe_evidence_is_refused(self):
        self.assertTrue(any('unsafe execution evidence' in e for e in self.errors_for(lambda x: x.update(evidence_files=['../outside']))))

    def test_markdown_status_mismatch_is_refused(self):
        self.assertTrue(any('Markdown execution inconsistent' in e for e in self.errors_for(lambda x: x.update(status='pending'))))

    def test_missing_first_task_execution_is_refused(self):
        data = copy.deepcopy(self.data)
        del data['tasks'][0]['execution']
        errors, _ = validator.validate(data, self.markdown)
        self.assertIn('first 30 tasks require execution metadata', errors)

if __name__ == '__main__':
    unittest.main()
