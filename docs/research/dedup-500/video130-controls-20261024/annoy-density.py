import json,sys,platform,importlib.metadata
from pathlib import Path
import numpy as np
from annoy import AnnoyIndex
rows=[]
for dimension in [2,500]:
 for value in [0.,1.]:
  for count in [3,21,257]:
   vector=[value]*dimension;index=AnnoyIndex(dimension,'euclidean');index.set_seed(1)
   for i in range(count):index.add_item(i,vector)
   index.build(10,n_jobs=1)
   for requested in [min(20,count),count]:
    ids,distances=index.get_nns_by_vector(vector,requested,search_k=20000,include_distances=True)
    assert len(set(ids))==len(ids) and all(d==0 for d in distances)
    rows.append(dict(dimension=dimension,value=value,count=count,requested=requested,returned=len(ids),ids=ids))
distinct=AnnoyIndex(2,'euclidean')
for i in range(8):distinct.add_item(i,[i,0])
distinct.build(10,n_jobs=1)
checks=[]
for requested in [1,2,3,5,8]:
 ids,distances=distinct.get_nns_by_vector([0,0],requested,search_k=20000,include_distances=True);checks.append(dict(requested=requested,ids=ids,distances=distances,index_items=distinct.get_n_items()))
report={'python':sys.version,'platform':platform.platform(),'annoy':importlib.metadata.version('annoy'),'numpy':np.__version__,'distinct_vector_checks':checks,'scope':'Direct native Annoy1.17.3 controlled seed1/single build worker;synthetic identical Euclidean vectors;not video/model recall','rows':rows};Path(__file__).with_name(sys.argv[1] if len(sys.argv)>1 else 'annoy-density-results.json').write_text(json.dumps(report,indent=2)+'\n')
print([{k:r[k] for k in ['dimension','value','count','requested','returned']} for r in rows])
