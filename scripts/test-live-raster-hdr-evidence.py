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

    def test_raw_and_model_source_identity_shape_and_credit(self):
        raster = 'native raster HDR surface: 64x48; pixel_aspect=Some(2.0); source=/test.heic'
        credit = 'native raster source credit after upload: 0'
        for source, released, check in [
            ('native RAW HDR surface: 2x3; samples=6; source=/test.cr3',
             'native RAW source credit after upload: 0',
             lambda log: module.validate_raw_log(log,0,'/test.cr3',2,3,6)),
            ('native model HDR surface: triangles=3; source=/test.obj',
             'native model source credit after upload: 0',
             lambda log: module.validate_model_log(log,0,'/test.obj',3))]:
            log = self.log.replace(raster,source).replace(credit,released)
            self.assertTrue(check(log))
            for changed in [log.replace(source,''), log.replace(released,''),
                            log.replace('after upload: 0','after upload: 4'),
                            log + '\n' + source, log + '\n' + released,
                            log.replace('source=/test.', 'source=/other.'),
                            log.replace('samples=6','samples=5').replace('triangles=3','triangles=2')]:
                self.assertFalse(check(changed))
            self.assertFalse(self.check(log))
    def test_native_empty_or_inconsistent_geometry_cannot_qualify(self):
        self.assertFalse(module.validate_raw_log(self.log,0,'/test.cr3',2,3,5))
        self.assertFalse(module.validate_raw_log(self.log,0,'/test.cr3',0,0,0))
        self.assertFalse(module.validate_model_log(self.log,0,'/test.obj',0))

unittest.main()
