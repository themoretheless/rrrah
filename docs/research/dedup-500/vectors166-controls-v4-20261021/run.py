"""Original classes with identity njit shim. Fault injection is explicit."""
from pathlib import Path
import sys,types,numpy as np
shim=types.ModuleType('numba');shim.njit=lambda f:f;sys.modules['numba']=shim
sys.path.insert(0,str(Path('.dedup-work/deduplicate166-20261021/src').resolve()))
from deduplicate_lib.plugins.duplicate_detection_algorithms.distance_matrix import DistanceMatrix
from deduplicate_lib.plugins.tolerance_calculators.perturbed_dataset_reclustering import PerturbedDatasetReclustering

d=DistanceMatrix(dataset_array=np.array([[0.],[10.]]),max_vector_array_size=8)
t=PerturbedDatasetReclustering(d,binary_search_steps=2)
try:t.calculate_tolerance()
except ValueError as e:
 assert 'zero-size array' in str(e)
 print('PASS default tolerance dataset stays empty and raises zero-size reduction:',str(e))
else:raise AssertionError('expected empty default failure')

class InjectedFailure(DistanceMatrix):
 def get_dataset_unique_structures(self):raise RuntimeError('injected count failure')
d=InjectedFailure(tolerance=.25,dataset_array=np.array([[0.],[10.]]),max_vector_array_size=8)
t=PerturbedDatasetReclustering(d,tolerance_dataset_array=np.array([[20.],[30.],[40.]]),binary_search_steps=2)
try:t.binary_search_tolerance(2)
except RuntimeError as e:assert str(e)=='injected count failure'
else:raise AssertionError('expected injected failure')
assert d.tolerance==.25
assert d.vector_count==3 and d.get_filled_dataset_array().tolist()==[[20.],[30.],[40.]]
print('PASS injected count failure restores scalar tolerance but leaves temporary dataset and vector count installed')
