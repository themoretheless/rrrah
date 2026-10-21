"""Re-execute prior controls with genuine Numba; remove only the identity shim line."""
from pathlib import Path
import importlib.metadata as md
import sys
for package in ['numpy','numba','llvmlite','scipy','pytest']:
 print('VERSION',package,md.version(package),flush=True)
for version in [2,3,4,6,7]:
 path=Path(f'docs/research/dedup-500/vectors166-controls-v{version}-20261021/run.py')
 lines=path.read_text().splitlines()
 removed=[line for line in lines if line.startswith('shim=types.ModuleType')]
 assert len(removed)==1
 source='\n'.join(line for line in lines if not line.startswith('shim=types.ModuleType'))+'\n'
 print('CONTROL VERSION',version,flush=True)
 exec(compile(source,str(path),'exec'),{'__name__':'__main__'})
 assert 'numba' in sys.modules and hasattr(sys.modules['numba'],'__version__')
print('PASS 12 prior full-class controls repeated with genuine Numba; no new unique cases',flush=True)
import numba,numpy as np
from deduplicate_lib.core.duplicate_detection_algorithm import euclidean_distance
assert numba.config.DISABLE_JIT==0
assert np.isclose(euclidean_distance(np.array([0.,0.]),np.array([1.,1.])),np.sqrt(2))
assert euclidean_distance.nopython_signatures
print('PASS JIT enabled, CPUDispatcher has native nopython signature',euclidean_distance.nopython_signatures,flush=True)
