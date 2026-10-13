#!/usr/bin/env python3
"""Guard collection evidence against incomplete or duplicated qualification logs."""
import runpy
import json
import unittest
from pathlib import Path

MODULE = runpy.run_path(str(Path(__file__).with_name('qualify-viewer-storage-gpu.py')))
VERIFY = MODULE['raster_collection_evidence']


def complete_log():
    rows = ['raster collection HDR oracle: 63x65; 8,-2,1,1; every binary16 pixel exact']
    declared = ['raster/cicp-' + name + '.png' for name in
                ('linear-2020', 'linear-p3', 'pq-2020', 'hlg-2020', 'narrow-709', 'encoded-p3')]
    declared += ['raster/gamma-chrm-709.png', 'raster/gamma-chrm-p3.png', 'raster/gamma-gray.png', 'raster/gamma-gray-packed-1.png', 'raster/gamma-gray-packed-2.png', 'raster/gamma-gray-packed-4.png']
    for source in range(118):
        name = (f'fixture-{source}' if source < 106 else
                declared[source - 106])
        count = 2 if source < 29 else 1
        for index in range(count):
            for mode in ('false', 'true'):
                key = f'{name}; index={index}; prepared={mode}'
                rows.append(f'raster collection: {key}; bits/color/frame exact; root=0; queue=0')
                rows.append(f'raster collection HDR: {key}; binary16 frame exact')
        rows.append(f'raster collection bounds: {name}; rejected_index={count}; retained_bytes_unchanged')
    rows.append('raster collection complete: sources=118; selections=147; storage_cases=294; root=0; queue=0')
    return rows


class CollectionEvidenceTests(unittest.TestCase):
    def test_gray_gamma_source_is_required(self):
        text = '\n'.join(complete_log())
        self.assertFalse(VERIFY(text.replace('raster/gamma-gray.png', 'unrelated.png')))

    def test_packed_gray_sources_are_required(self):
        text = '\n'.join(complete_log())
        for depth in (1,2,4):
            self.assertFalse(VERIFY(text.replace(f'raster/gamma-gray-packed-{depth}.png', 'unrelated.png')))

    def test_complete(self):
        self.assertTrue(VERIFY('\n'.join(complete_log())))

    def test_cicp_sources_cannot_be_replaced_by_unrelated_cases(self):
        text = '\n'.join(complete_log())
        for name in ('linear-2020', 'linear-p3', 'pq-2020', 'hlg-2020', 'narrow-709', 'encoded-p3'):
            self.assertFalse(VERIFY(text.replace(f'raster/cicp-{name}.png', 'unrelated.png')))

    def test_gamma_chroma_sources_are_required(self):
        text = '\n'.join(complete_log())
        for name in ('709', 'p3'):
            self.assertFalse(VERIFY(text.replace(f'raster/gamma-chrm-{name}.png', 'unrelated.png')))

    def test_missing_or_duplicate_case_and_bounds(self):
        for prefix in ('raster collection: ', 'raster collection HDR: ', 'raster collection bounds: '):
            for duplicate in (False, True):
                with self.subTest(prefix=prefix, duplicate=duplicate):
                    rows = complete_log()
                    index = next(i for i, row in enumerate(rows) if row.startswith(prefix))
                    row = rows[index]
                    if duplicate:
                        rows.append(row)
                    else:
                        rows.pop(index)
                    self.assertFalse(VERIFY('\n'.join(rows)))

    def test_missing_control_or_stale_summary(self):
        rows = complete_log()
        self.assertFalse(VERIFY('\n'.join(rows[1:])))
        rows[-1] = rows[-1].replace('118', '65').replace('147', '94').replace('294', '188')
        self.assertFalse(VERIFY('\n'.join(rows)))


class RawWorkerEvidenceTests(unittest.TestCase):
    def complete_log(self):
        rows = ['RAW worker navigation adapter: Metal synthetic',
                'RAW worker navigation: 1920x1080 Metal frame exact',
                'RAW worker navigation: 6188x4120 Metal frame exact',
                'reverse prefetch: 25494560 samples exact; cancelled forward absent; one backward entry; peak=10 final=0']
        rows += [f'RAW worker navigation: direction-index={index}; 6188x4120 frame and original samples exact; disk-count=1; final=0'
                 for index in (2, 0)]
        rows += [f'RAW worker HDR: {size}; all scene-linear binary16 components exact and finite'
                 for size in ('1920x1080', '6188x4120', '6188x4120', '6188x4120')]
        return rows

    def test_complete(self):
        self.assertTrue(MODULE['raw_worker_navigation_evidence']('\n'.join(self.complete_log()), 1, 0))

    def test_missing_duplicate_or_sdr_only(self):
        rows = self.complete_log()
        verify = MODULE['raw_worker_navigation_evidence']
        for index in range(len(rows)):
            with self.subTest(missing=index):
                self.assertFalse(verify('\n'.join(rows[:index] + rows[index+1:]), 1, 0))
        self.assertFalse(verify('\n'.join(rows + [rows[-1]]), 1, 0))
        self.assertFalse(verify('\n'.join(rows[:-4]), 1, 0))
        self.assertFalse(verify('\n'.join(rows), 1, 1))


class TimingEvidenceTests(unittest.TestCase):
    def payload(self):
        return {'median_gpu_ms': 15.5, 'p95_gpu_ms': 29., 'max_gpu_ms': 30.,
                'samples_gpu_ms': list(range(1, 31)), 'warmups': 5,
                'source': [4096, 4096], 'target': [2048, 2048], 'target_format': 'Rgba16Float'}

    def text(self, row):
        return 'backend: Metal\n' + json.dumps(row)

    def test_complete(self):
        self.assertTrue(MODULE['raster_timing_evidence'](self.text(self.payload()), 1, 0))

    def test_invalid_distribution_or_scope(self):
        verify = MODULE['raster_timing_evidence']
        for key, bad in [('p95_gpu_ms', 28.), ('max_gpu_ms', 29.), ('median_gpu_ms', 1.),
                         ('samples_gpu_ms', [1.] * 29), ('samples_gpu_ms', [float('nan')] * 30),
                         ('warmups', 0), ('target_format', 'Rgba8UnormSrgb')]:
            with self.subTest(key=key, bad=bad):
                row = self.payload()
                row[key] = bad
                self.assertFalse(verify(self.text(row), 1, 0))
        text = self.text(self.payload())
        self.assertFalse(verify(text.replace('Metal', 'Vulkan'), 1, 0))
        self.assertFalse(verify(text, 0, 1))
        self.assertFalse(verify(text + '\n' + json.dumps(self.payload()), 1, 0))


if __name__ == '__main__':
    unittest.main()
