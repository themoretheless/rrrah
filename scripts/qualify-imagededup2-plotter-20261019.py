"""Original plotting module/tests in noninteractive Agg backend and owned output directory."""
from pathlib import Path
from types import ModuleType
import hashlib
import importlib.util
import json
import os
import sys
import xml.etree.ElementTree as ET

R = Path(__file__).resolve().parents[1]
S = R / '.dedup-work/imagededup2-20261019/source'
W = R / '.dedup-work/imagededup2-plotter-20261019'
W.mkdir(exist_ok=False)
os.environ['MPLCONFIGDIR'] = str(W / 'mplconfig')
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from PIL import Image
import pytest
for name, relative in [('imagededup', 'imagededup'), ('imagededup.utils', 'imagededup/utils')]:
    package = ModuleType(name)
    package.__path__ = [str(S / relative)]
    sys.modules[name] = package
sys.modules['imagededup'].utils = sys.modules['imagededup.utils']
path = S / 'imagededup/utils/plotter.py'
spec = importlib.util.spec_from_file_location('imagededup.utils.plotter', path)
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)
sys.modules['imagededup.utils'].plotter = module
tests = S / 'tests/test_plotter.py'
try:
    os.chdir(S)
    code = pytest.main(['-c', '/dev/null', '--confcutdir=' + str(S / 'tests'), '-q',
                        '-p', 'no:cacheprovider', '--junitxml=' + str(W / 'junit.xml'), str(tests)])
finally:
    os.chdir(R)
assert code == 0
fixtures = W / 'fixtures'
fixtures.mkdir()
Image.new('RGB', (16, 12), 'red').save(fixtures / 'a.png')
Image.new('RGB', (16, 12), 'blue').save(fixtures / 'b.png')
out = W / 'plot.png'
module.plot_duplicates(fixtures, {'a.png': ['b.png']}, 'a.png', str(out))
with Image.open(out) as image:
    dimensions = list(image.size)
assert out.stat().st_size > 0
suite = ET.parse(W / 'junit.xml').getroot().find('testsuite')
result = dict(status='passed_original_plotter_suite_and_Agg_output',
    source_pins={str(p.relative_to(S)): hashlib.sha256(p.read_bytes()).hexdigest() for p in [path, tests]},
    original_suite={k: suite.attrib[k] for k in ['tests', 'failures', 'errors', 'skipped']},
    matplotlib_version=matplotlib.__version__, output_dimensions=dimensions,
    findings=['Original _plot_images allocates Figure10x14 but uses pyplot.subplot/savefig on a separate current figure; actual output dimensions recorded.',
              'Upstream plotter tests exercise validation/dispatch but leave integrated rendering test commented out.',
              'Original loader uses Image.open directly without EXIF transpose, unlike an orientation-normalized preview contract.'],
    limits=['Entire original plotter module/tests unchanged with namespace-package routing; real matplotlib Agg/Pillow/pytest.',
            'Synthetic two-image PNG output generated; visual layout inspection pending, no GUI/interactive/platform proof.',
            'No foreign production code adopted; pHash native investigation remains deferred.'])
(W / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps({'suite': result['original_suite'], 'output_dimensions': dimensions}))
