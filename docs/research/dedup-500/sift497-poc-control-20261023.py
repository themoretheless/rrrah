import importlib.util,sys,tempfile
from pathlib import Path
root=Path('/Users/themoretheless/Documents/Sources/rrrah/.dedup-work/sift497-20261023/POC/scripts');sys.path.insert(0,str(root))
import numpy as np
# SciPy unavailable: explicit distance adapter only, original clustering unchanged.
import types
scipy=types.ModuleType('scipy');spatial=types.ModuleType('scipy.spatial');distance=types.ModuleType('scipy.spatial.distance')
def pdist(v,metric):
 assert metric=='hamming'
 return np.array([np.count_nonzero(v[i]!=v[j])/len(v[i]) for i in range(len(v)) for j in range(i+1,len(v))])
def squareform(d):
 n=int((1+(1+8*len(d))**0.5)/2);out=np.zeros((n,n));k=0
 for i in range(n):
  for j in range(i+1,n):out[i,j]=out[j,i]=d[k];k+=1
 return out
distance.pdist=pdist;distance.squareform=squareform
sys.modules.update({'scipy':scipy,'scipy.spatial':spatial,'scipy.spatial.distance':distance})
from cluster_utils import cluster_hashes,get_frame_indices
from io_utils import save_groups
A=('A',np.array([0,0]));B=('B',np.array([0,1]));C=('C',np.array([1,1]))
a=cluster_hashes([A,B,C],1);b=cluster_hashes([B,A,C],1)
assert a==[['A','B']] and b==[['B','A','C']]
assert np.count_nonzero(A[1]!=C[1])==2
assert get_frame_indices(2,8)=={0,1}
with tempfile.TemporaryDirectory(prefix='sift-poc-review-') as td:
 p=Path(td);inp=p/'input';inp.mkdir();(inp/'a').write_text('controlled');out=p/'output';save_groups([['a']],str(inp),str(out),True);assert (inp/'a').exists() and (out/'group_1').is_dir()
try:cluster_hashes([],1)
except IndexError:pass
else:raise AssertionError('expected empty input failure')
print('original POC: order-dependent anchor grouping; group diameter exceeds threshold; empty list IndexError; dry run creates output directories')
