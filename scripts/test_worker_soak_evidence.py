"""Reject partial, short, skipped or contradictory real-worker soak evidence."""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location(
    'worker_soak', Path(__file__).with_name('run-metal-collection-soak.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
ROOT = Path(__file__).resolve().parents[1]


class WorkerSoakEvidence(unittest.TestCase):
    def setUp(self):
        self.text = (ROOT / 'docs/research/metal-prefetch-boundaries-2026-11-02/window-soak.log').read_text()

    def test_actual_log_and_duration(self):
        self.assertTrue(module.worker_evidence(self.text, 60))
        self.assertFalse(module.worker_evidence(self.text, 61))

    def test_missing_or_duplicate_contract_lines(self):
        lines = [line for line in self.text.splitlines(keepends=True)
                 if any(marker in line for marker in ('large soak adapter:',
                         'dynamic prefetch windows=', 'large real worker soak PASS:'))]
        self.assertEqual(len(lines), 3)
        for line in lines:
            self.assertFalse(module.worker_evidence(self.text.replace(line, '', 1), 60))
            self.assertFalse(module.worker_evidence(self.text + line, 60))

    def test_wrong_accounting_backend_windows_or_output(self):
        for before, after in [('Metal ', 'Vulkan '), ('final=0', 'final=1'),
                              ('root_peak=134217728', 'root_peak=134217729'),
                              ('largest_settled=100663296', 'largest_settled=100663297'),
                              ('seconds=60.035', 'seconds=inf'), ('sources=64', 'sources=1'), ('dimensions=1024x1024', 'dimensions=1x1'),
                              ('prefetch=true', 'prefetch=false'),
                              ('dynamic_window=true', 'dynamic_window=false'),
                              ('mixed_ppm_tga_bmp=true', 'mixed_ppm_tga_bmp=false'),
                              ('selections=392', 'selections=393'),
                              ('[64, 64, 64, 56, 48, 48, 48]', '[64, 64, 64, 56, 48, 48, 0]'),
                              ('18446744073709551615', '100'),
                              ('swap_objects<=3', 'swap_objects<=4'),
                              ('exact Metal SDR/HDR every selection', 'SDR only')]:
            with self.subTest(before=before):
                changed = self.text.replace(before, after)
                self.assertNotEqual(changed, self.text)
                self.assertFalse(module.worker_evidence(changed, 60))


if __name__ == '__main__':
    unittest.main()
