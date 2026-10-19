"""Inspect original build logic through captured setup calls; never build or release."""
from pathlib import Path
from types import SimpleNamespace
import ast
import hashlib
import json
import tomllib

R = Path(__file__).resolve().parents[1]
S = R / '.dedup-work/imagededup2-20261019/source'
W = R / '.dedup-work/imagededup2-packaging-20261019'
W.mkdir(exist_ok=False)
names = ['setup.py', 'pyproject.toml', 'requirements.txt', 'MANIFEST.in',
         'imagededup/__init__.py', 'imagededup/methods/__init__.py',
         'imagededup/evaluation/__init__.py', 'imagededup/handlers/__init__.py',
         'imagededup/handlers/search/__init__.py', 'imagededup/handlers/metrics/__init__.py',
         'imagededup/utils/__init__.py', '.github/workflows/test.yml',
         '.github/workflows/build_wheels.yml', 'new_release.sh',
         'mkdocs/docs/user_guide/windows.md']
pins = {n: hashlib.sha256((S / n).read_bytes()).hexdigest() for n in names}
parsed = ast.parse((S / 'setup.py').read_text())
# Preserve all setup calculations; route imports/build availability and final calls explicitly.
nodes = [n for n in parsed.body if not isinstance(n, (ast.Import, ast.ImportFrom, ast.Try))]
matrix = []
for platform in ['linux', 'darwin', 'win32']:
    for cython in [False, True]:
        captures = []
        env = dict(sys=SimpleNamespace(platform=platform), use_cython=cython,
                   Extension=lambda *args, **kwargs: {'args': args, 'kwargs': kwargs},
                   cythonize=lambda extensions: extensions,
                   setup=lambda **kwargs: captures.append(kwargs))
        exec(compile(ast.Module(body=nodes, type_ignores=[]), 'original_setup_calculations', 'exec'), env)
        assert len(captures) == 1
        extension = captures[0]['ext_modules'][0]
        args = extension['kwargs']['extra_compile_args']
        assert ('-march=native' in args) == (platform == 'linux')
        assert extension['args'][1][0].endswith('.pyx' if cython else '.cpp')
        matrix.append(dict(platform=platform, cython_available=cython, extension=extension))
metadata = tomllib.loads((S / 'pyproject.toml').read_text())
assert not any(d.split('>')[0].split('=')[0].lower() == 'numpy' for d in metadata['project']['dependencies'])
assert 'numpy==1.26.4' in (S / 'requirements.txt').read_text().splitlines()
version = ast.literal_eval(ast.parse((S / 'imagededup/__init__.py').read_text()).body[0].value)
assert version == metadata['project']['version']
result = dict(status='passed_captured_build_logic_controls_not_real_build', source_pins=pins,
    captured_platform_cython_matrix=matrix, version_consistent=version,
    findings=['Linux compile/link options include march=native and mtune=native; wheel portability needs actual artifact/CPU verification.',
              'NumPy1.26.4 pin belongs to requirements.txt, not project.dependencies consumed by pip install .; direct imports rely on transitive NumPy installation.',
              'methods initializer eagerly imports CNN as well as hashing; utils initializer imports plotting and model helpers.',
              'Test workflow declares3OSx4Python versions; declaration alone is not evidence those jobs passed.',
              'Wheel workflow disables CIBW_TEST_COMMAND on Windows, although separate test workflow includes Windows.',
              'Windows guide documents multiprocessing main-guard requirement; zero requested workers is clamped to one Pool worker by general_utils.',
              'Release helper includes git add ., commit and push; read only, never executed.'],
    limits=['Captured Extension/setup and identity cythonize callbacks; imports/availability routed explicitly. No compiler, wheel, install, publish or platform execution.',
            'Pinned source snapshot only, not current upstream CI status.',
            'New15wholefiles read; earlier12files retained, repository still partial.',
            'pHash native investigation deferred; no production code adopted.'])
(W / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps({'captured_configurations': len(matrix), 'whole_files_read': len(names), 'version': version}))
