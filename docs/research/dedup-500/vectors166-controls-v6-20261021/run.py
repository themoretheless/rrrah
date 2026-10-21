"""Original plateau class, NumPy, identity njit shim. Injected state is explicit."""
from pathlib import Path
import sys,types,numpy as np
shim=types.ModuleType('numba');shim.njit=lambda f:f;sys.modules['numba']=shim
sys.path.insert(0,str(Path('.dedup-work/deduplicate166-20261021/src').resolve()))
from deduplicate_lib.plugins.duplicate_detection_algorithms.distance_matrix import DistanceMatrix
from deduplicate_lib.plugins.tolerance_calculators.natural_tolerance_plateau_probe import NaturalTolerancePlateauProbe

def make():
 d=DistanceMatrix(dataset_array=np.array([[0.],[10.]]),max_vector_array_size=8)
 return d,NaturalTolerancePlateauProbe(d,datapoints_to_calculate_gradient=2)
d,p=make()
counts={float(i):3 for i in range(6)}
mask=p.get_plateau_log(sorted(counts),counts)
assert mask.tolist()==[False]*4
assert p.find_plateaus(counts)==[]
print('PASS constant count curve has zero gradient but detects no plateau (strict zero threshold)')
d,p=make()
p.get_plateau_log=lambda *args:np.ones(4,dtype=bool)
assert p.find_plateaus({float(i):6-i for i in range(6)})==[]
print('PASS explicitly injected all-true plateau mask is lost by original interval extraction')
d,p=make()
try:p.calculate_tolerance()
except ValueError as e:
 assert 'zero-size array' in str(e)
 print('PASS default plateau calculator with nonempty DDA fails empty tolerance dataset reduction')
else:raise AssertionError('expected empty reduction')
class Failure(DistanceMatrix):
 def get_dataset_unique_structures(self):raise RuntimeError('injected probe failure')
d=Failure(tolerance=.25,dataset_array=np.array([[0.],[10.]]),max_vector_array_size=8)
p=NaturalTolerancePlateauProbe(d,tolerance_dataset_array=np.array([[20.],[30.],[40.]]))
try:p.tolerance_probe(.1,1.,3)
except RuntimeError as e:assert str(e)=='injected probe failure'
else:raise AssertionError('expected injected failure')
assert d.tolerance==.25 and d.vector_count==3
assert d.get_filled_dataset_array().tolist()==[[20.],[30.],[40.]]
print('PASS injected probe failure restores scalar tolerance but leaves temporary dataset installed')
