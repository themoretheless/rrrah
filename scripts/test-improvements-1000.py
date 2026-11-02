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

    def test_stale_report_link_is_refused(self):
        data = copy.deepcopy(self.data)
        task = data['tasks'][23]
        old = Path(task['execution']['report']).relative_to('docs').as_posix()
        markdown = self.markdown.replace(f'[Отчёт]({old})', '[Отчёт](research/first30-2026-10-14/REPORT.md)')
        errors, _ = validator.validate(data, markdown)
        self.assertIn('RR-0024: Markdown report link inconsistent', errors)

    def test_missing_first_task_execution_is_refused(self):
        data = copy.deepcopy(self.data)
        del data['tasks'][0]['execution']
        errors, _ = validator.validate(data, self.markdown)
        self.assertIn('first 30 tasks require execution metadata', errors)

    def batch_errors(self, change):
        data = copy.deepcopy(self.data)
        data['execution_batches'] = [{'ids': ['RR-0001'], 'summary': {'verified': 1},
                                     'report': data['tasks'][0]['execution']['report']}]
        change(data['execution_batches'][0])
        return validator.validate(data, self.markdown)[0]

    def test_batch_summary_mismatch_is_refused(self):
        self.assertIn('execution batch summary inconsistent', self.batch_errors(lambda b: b.update(summary={})))

    def test_batch_duplicate_and_unknown_ids_are_refused(self):
        for ids in [['RR-0001', 'RR-0001'], ['RR-9999']]:
            self.assertIn('execution batch IDs must be existing and unique', self.batch_errors(lambda b: b.update(ids=ids)))

    def test_batch_missing_execution_is_refused(self):
        data=copy.deepcopy(self.data)
        data['execution_batches']=[{'ids':['RR-1000'],'summary':{},'report':data['tasks'][0]['execution']['report']}]
        data['tasks'][-1].pop('execution',None)
        self.assertIn('execution batch task missing execution',validator.validate(data,self.markdown)[0])

    def test_batch_report_outside_repository_is_refused(self):
        self.assertIn('execution batch report missing or unsafe',self.batch_errors(lambda b:b.update(report='../outside')))

if __name__ == '__main__':
    unittest.main()
