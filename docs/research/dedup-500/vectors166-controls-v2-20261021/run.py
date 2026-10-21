"""Whole original classes/base imported; explicit identity njit adapter, actual NumPy.
Not Numba machine-code execution or original dependency environment.
"""
from pathlib import Path
import sys,types,numpy as np
shim=types.ModuleType('numba');shim.njit=lambda f:f;sys.modules['numba']=shim
sys.path.insert(0,str(Path('.dedup-work/deduplicate166-20261021/src').resolve()))
from deduplicate_lib.plugins.duplicate_detection_algorithms.distance_matrix import DistanceMatrix
for data,count,mask in [([[0.],[10.],[10.]],1,[True,False,False]),([[10.],[10.],[0.]],2,[True,False,True])]:
 d=DistanceMatrix(tolerance=.5,dataset_array=np.array(data),max_vector_array_size=8)
 assert d.get_dataset_unique_structures()==count and d.unique_vector_indices.tolist()==mask
 print('PASS full original class representative mask',mask)
d=DistanceMatrix(tolerance=1.5,dataset_array=np.array([[0.,0.],[1.,1.]]),max_vector_array_size=8)
assert d.get_dataset_unique_structures()==1
d.distance_metric='manhattan'
assert d.calculate_distance(np.array([0.,0.]),np.array([1.,1.]))==2
assert d.get_dataset_unique_structures()==1
fresh=DistanceMatrix(tolerance=1.5,dataset_array=np.array([[0.,0.],[1.,1.]]),distance_metric='manhattan',max_vector_array_size=8)
assert fresh.get_dataset_unique_structures()==2
print('PASS metric change retains stale Euclidean matrix; fresh Manhattan yields2')
