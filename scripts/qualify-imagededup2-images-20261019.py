"""Selected original image-utils module/tests and independent synthetic decode controls."""
from pathlib import Path
from types import ModuleType
import hashlib
import importlib.util
import json
import sys
import numpy as np
from PIL import Image, ImageOps
import PIL
import pytest

R = Path(__file__).resolve().parents[1]
S = R / '.dedup-work/imagededup2-20261019/source'
W = R / '.dedup-work/imagededup2-images-20261019'
W.mkdir(exist_ok=False)
for name, directory in [('imagededup', 'imagededup'), ('imagededup.utils', 'imagededup/utils')]:
    package = ModuleType(name)
    package.__path__ = [str(S / directory)]
    sys.modules[name] = package
sys.modules['imagededup'].utils = sys.modules['imagededup.utils']
name = 'imagededup.utils.image_utils'
source = S / 'imagededup/utils/image_utils.py'
spec = importlib.util.spec_from_file_location(name, source)
module = importlib.util.module_from_spec(spec)
sys.modules[name] = module
spec.loader.exec_module(module)
sys.modules['imagededup.utils'].image_utils = module
tests = S / 'tests/test_image_utils.py'
code = pytest.main(['-c', '/dev/null', '--confcutdir=' + str(S / 'tests'),
                    '-q', '-p', 'no:cacheprovider', '--junitxml=' + str(W / 'junit.xml'), str(tests)])
assert code == 0, code
base = np.arange(3 * 5 * 3, dtype=np.uint8).reshape(3, 5, 3)
orientations = []
for orientation in range(1, 9):
    image = Image.fromarray(base)
    exif = Image.Exif()
    exif[274] = orientation
    path = W / f'orientation-{orientation}.png'
    image.save(path, exif=exif)
    actual = module.load_image(path)
    with Image.open(path) as reopened:
        expected = np.asarray(ImageOps.exif_transpose(reopened))
    matches = actual.shape == expected.shape and np.array_equal(actual, expected)
    assert np.array_equal(actual, base)
    assert matches == (orientation == 1)
    orientations.append(dict(orientation=orientation, loaded_shape=list(actual.shape),
                             oriented_shape=list(expected.shape), matches_oriented=matches))
alpha_arrays = []
for color in [(255, 0, 0, 0), (0, 0, 255, 0)]:
    path = W / f'alpha-{color[0]}.png'
    Image.new('RGBA', (5, 3), color).save(path)
    alpha_arrays.append(module.load_image(path))
assert not np.array_equal(*alpha_arrays)
animations = []
for color in ['blue', 'green']:
    path = W / f'animation-{color}.gif'
    Image.new('RGB', (5, 3), 'red').save(path, save_all=True,
        append_images=[Image.new('RGB', (5, 3), color)], duration=100, loop=0)
    animations.append(module.load_image(path))
assert np.array_equal(*animations)
corrupt = W / 'corrupt.png'
corrupt.write_bytes(b'not an image')
assert module.load_image(corrupt) is None
values = np.array([[0, 255, 256, 65535]], dtype=np.uint16)
assert module.preprocess_image(values).tolist() == [[0, 255, 0, 255]]
import xml.etree.ElementTree as ET
suite = ET.parse(W / 'junit.xml').getroot().find('testsuite')
result = dict(status='passed_original_selected_module_suite_and_characterization',
    source_pins={str(p.relative_to(S)): hashlib.sha256(p.read_bytes()).hexdigest() for p in [source, tests]},
    original_suite={k: suite.attrib[k] for k in ['tests', 'failures', 'errors', 'skipped']},
    environment={'numpy': np.__version__, 'Pillow': PIL.__version__, 'pytest': pytest.__version__},
    orientation_controls=orientations, alpha_control='different invisible RGB values remain different after alpha discarded',
    animation_control='two GIFs with equal first frames and different second frames load as equal arrays',
    uint16_array_control='preprocess casts to uint8 modulo256; no range normalization',
    corrupt_control='returns None',
    limits=['Entire original image_utils module unchanged; namespace package routing bypasses package initializer/CNN imports.',
            'Actual original logger, real NumPy/Pillow/pytest and pytest-mock3.15.1; full original test_image_utils target only.',
            'Lossless synthetic PNG EXIF controls use Pillow exif_transpose oracle; not JPEG/CMYK/all-format qualification.',
            'No upstream matching/CNN/hash generation/full-package/runtime/platform/performance qualification.',
            'No foreign production code adopted; pHash native investigation deferred.'])
(W / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result['original_suite']))
