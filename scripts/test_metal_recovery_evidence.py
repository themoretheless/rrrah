#!/usr/bin/env python3
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('qualification', Path(__file__).with_name('qualify-viewer-storage-gpu.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


def upload_log():
    lines = ['large upload cancellation adapter: Metal test']
    for fmt, initial, retry in [
        ('Rgba8UnormSrgb', '[255, 0, 0, 255]', '[0, 0, 255, 255]'),
        ('Rgba16Float', '[17408, 47104, 0, 15360]', '[46080, 0, 17408, 15360]'),
    ]:
        lines += [
            'raster upload pressure: texture-and-queue-refused; previous-frame-exact; temporary-credit-zero',
            'raster upload cancellation: total_checkpoints=2933; exercised=[1, 2, 1466, 2931, 2932, 2933]; upload_budget=192000000',
            f'raster replacement values: format={fmt}; initial_center={initial}; retry_center={retry}',
            f'raster replacement recovery: format={fmt}; pressure/cancellation previous frame exact; retry blue exact; final GPU/queue credit=0',
        ]
    return '\n'.join(lines) + '\n'


def selected_log():
    return ('Selected-page admitted cancellation adapter: Metal test\n'
            'Selected-page admitted cancellation: bytes=67108864; obsolete page1 unpublished; RAM=0; page2 retry payload/SDR/HDR exact; blocked decoder bypassed; final=0\n')


class RecoveryEvidence(unittest.TestCase):
    def cases(self):
        return [(module.signed_upload_recovery_evidence, upload_log()),
                (module.selected_page_cancel_evidence, selected_log())]

    def test_complete(self):
        for check, text in self.cases():
            self.assertTrue(check(text, 1, 0))

    def test_missing_duplicate_or_skipped(self):
        for check, text in self.cases():
            for line in text.splitlines(keepends=True):
                self.assertFalse(check(text.replace(line, '', 1), 1, 0))
                self.assertFalse(check(text + line, 1, 0))
            for passed, ignored in [(0, 0), (2, 0), (1, 1)]:
                self.assertFalse(check(text, passed, ignored))

    def test_wrong_backend_values_or_accounting(self):
        for old, new in [('Metal ', 'Vulkan '), ('17408', '15360'),
                         ('47104', '0'), ('192000000', '16'),
                         ('2933', '3'), ('credit=0', 'credit=16')]:
            self.assertFalse(module.signed_upload_recovery_evidence(upload_log().replace(old, new), 1, 0))
        for old, new in [('Metal ', 'Vulkan '), ('67108864', '16'), ('RAM=0', 'RAM=1'),
                         ('final=0', 'final=16'), ('page2', 'page1'), ('unpublished', 'published')]:
            self.assertFalse(module.selected_page_cancel_evidence(selected_log().replace(old, new), 1, 0))


if __name__ == '__main__':
    unittest.main()
