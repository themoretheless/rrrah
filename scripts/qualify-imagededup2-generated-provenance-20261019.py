"""Regenerate full native C++ with pinned Cython and compare every byte to tracked output."""
from pathlib import Path
import difflib
import hashlib
import json
import os
import shutil
import Cython
from Cython.Build import cythonize
from setuptools import Extension

R = Path(__file__).resolve().parents[1]
S = R / '.dedup-work/imagededup2-20261019/source'
W = R / '.dedup-work/imagededup2-generated-provenance-20261019'
W.mkdir(exist_ok=False)
relative = 'imagededup/handlers/search/brute_force_cython_ext.cpp'
original = S / relative
metadata = json.loads(original.read_text().split('/* BEGIN: Cython Metadata\n',1)[1].split('\nEND: Cython Metadata */',1)[0])
assert Cython.__version__ == '3.1.3'
for name in ['imagededup/handlers/search/brute_force_cython_ext.pyx',
             'imagededup/handlers/search/builtin/builtin.h']:
    target = W / name
    target.parent.mkdir(parents=True,exist_ok=True)
    shutil.copy2(S/name,target)
old = Path.cwd()
try:
    os.chdir(W)
    extension = Extension(**metadata['distutils'])
    cythonize([extension], force=True)
finally:
    os.chdir(old)
generated = W / relative
same = generated.read_bytes() == original.read_bytes()
diff = ''.join(difflib.unified_diff(original.read_text().splitlines(keepends=True),
    generated.read_text().splitlines(keepends=True),fromfile='tracked',tofile='regenerated'))
(W/'regeneration.diff').write_text(diff)
result=dict(status='byte_identical_regeneration' if same else 'regeneration_diff_requires_review',
    cython_version=Cython.__version__, tracked_sha256=hashlib.sha256(original.read_bytes()).hexdigest(),
    regenerated_sha256=hashlib.sha256(generated.read_bytes()).hexdigest(),
    byte_identical=same,tracked_lines=len(original.read_text().splitlines()),
    metadata=metadata,
    limits=['Regeneration in owned copy; original checkout untouched.',
            'Generated-file provenance check, not manual reading of every Cython runtime helper or other-platform proof.',
            'Prior unchanged original binary compilation/native oracle evidence separate; no new memory-safety or performance claim.',
            'No production code adopted; deferred native pHash investigation untouched.'])
(W/'result.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'byte_identical':same,'tracked_lines':result['tracked_lines'],'diff_bytes':len(diff.encode())}))
