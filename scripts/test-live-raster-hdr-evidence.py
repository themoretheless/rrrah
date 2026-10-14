#!/usr/bin/env python3
"""Synthetic verifier tests; these do not constitute GPU/HDR evidence."""
import importlib.util
from pathlib import Path
import unittest
spec = importlib.util.spec_from_file_location('live', Path(__file__).with_name('qualify-live-raster-hdr.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

class Evidence(unittest.TestCase):
    log = '\n'.join([
        'adapter: AdapterInfo { backend: Metal, name: "test" }',
        'native raster HDR surface: 64x48; pixel_aspect=Some(2.0); source=/test.heic',
        'native raster source credit after upload: 0',
        'display after present: frames=90 elapsed=1.7s DisplayHdrInfo { headroom: Some(DisplayHeadroom { current: Some(8.0) }), coarse: Some(DisplayCoarseRange { high_dynamic_range: Some(true) }) }',
        'presentation complete: frames=160 elapsed=3.100s',
        'live resize presentation: confirmed=true frames_after_resize=130',
        'presented: RGBA16Float / ExtendedSrgbLinear; no validation errors',
    ])
    def check(self, log, code=0):
        return module.validate_log(log, code, '/test.heic', 64, 48, 2.)
    def test_complete(self):
        self.assertTrue(self.check(self.log))
    def test_reject_missing_evidence(self):
        for line in self.log.splitlines():
            self.assertFalse(self.check(self.log.replace(line, '')), line)
    def test_reject_wrong_values(self):
        for before, after in [('Metal', 'Vulkan'), ('64x48', '48x64'), ('Some(2.0)', 'Some(1.0)'),
                              ('/test.heic', '/other.heic'), ('frames=160', 'frames=1'),
                              ('3.100s', '0.100s'), ('high_dynamic_range: Some(true)', 'high_dynamic_range: Some(false)'), ('current: Some(8.0)', 'current: Some(1.0)'), ('frames_after_resize=130', 'frames_after_resize=0')]:
            self.assertFalse(self.check(self.log.replace(before, after)))
    def test_hdr_activation_and_headroom_must_agree_in_one_frame(self):
        inactive_high = self.log.replace('high_dynamic_range: Some(true)', 'high_dynamic_range: Some(false)')
        active_low = next(line for line in self.log.splitlines() if line.startswith('display after present:')).replace('current: Some(8.0)', 'current: Some(1.0)')
        self.assertFalse(self.check(inactive_high + '\n' + active_low))
    def test_reject_failure_or_duplicates(self):
        self.assertFalse(self.check(self.log, 1))
        self.assertFalse(self.check(self.log, None))
        self.assertFalse(self.check(self.log + '\npresentation complete: frames=160 elapsed=3.100s'))

unittest.main()
