from pathlib import Path
import json,hashlib
import numpy as np
C=Path('.dedup-work/gatherer433-20261021');s=(C/'rotateCrop.py').read_text();start=s.index('def order_points(');end=s.index('\ndef four_point_transform(',start);ns={'np':np};exec(compile(s[start:end],'original-order-points','exec'),ns)
p=np.array([[0,1],[1,0],[2,1],[1,2]],dtype=np.float32);result=ns['order_points'](p);unique=len(np.unique(result,axis=0));assert unique==3
r={'original_function_executed_unchanged':True,'source_sha256':hashlib.sha256(s.encode()).hexdigest(),'input':p.tolist(),'output':result.tolist(),'unique_output_corners':unique,'expected':4,'limits':['Only isolated original order_points executed; no OpenCV transform, database, network scraper or top-level module execution.','Diamond sum/difference ties demonstrate degeneracy; not a complete quadrilateral ordering oracle.']};Path('docs/research/dedup-500/gatherer433-controls-20261021/result.json').write_text(json.dumps(r,indent=2)+'\n');print(json.dumps(r))
