"""All8-bit YBR tuples: independent exact Fraction full-range BT.601 inverse.
Derives RGB from Kr=0.299, Kb=0.114; round-half-up, then saturate.
Also records (does not substitute) pydicom3.0.2 float32 disagreements.
"""
from pathlib import Path
from fractions import Fraction
import hashlib,json,sys,warnings
import numpy as np
from pydicom.pixels import convert_color_space
path=Path(sys.argv[1]);path.parent.mkdir(parents=True,exist_ok=True)
kr,kb=Fraction('0.299'),Fraction('0.114');kg=1-kr-kb
round_up=lambda x:(2*x.numerator+x.denominator)//(2*x.denominator)
offsets=[]
for cb in range(256):
 for cr in range(256):
  r=2*(1-kr)*(cr-128);b=2*(1-kb)*(cb-128);g=(-kr*r-kb*b)/kg
  offsets.append([round_up(r),round_up(g),round_up(b)])
offsets=np.array(offsets,dtype=np.int32)
cb,cr=np.indices((256,256));cb=cb.reshape(-1);cr=cr.reshape(-1)
differences=0;examples=[];warning_messages=set();digest=hashlib.sha256()
with path.open('wb') as output:
 for y in range(256):
  exact=np.clip(offsets+y,0,255).astype(np.uint8)
  data=exact.tobytes();output.write(data);digest.update(data)
  source=np.stack([np.full_like(cb,y),cb,cr],axis=1).astype(np.uint8)
  with warnings.catch_warnings(record=True) as recorded:
   reference=convert_color_space(source,'YBR_FULL','RGB')
  warning_messages.update(str(w.message) for w in recorded)
  bad=np.any(exact!=reference,axis=1);differences+=int(np.count_nonzero(bad))
  for i in np.flatnonzero(bad)[:max(0,12-len(examples))]:examples.append({'source':source[i].tolist(),'exact':exact[i].tolist(),'pydicom':reference[i].tolist()})
report={'oracle':str(path),'triples':256**3,'bytes':path.stat().st_size,'sha256':digest.hexdigest(),'source_model':'exact Kr=299/1000, Kb=114/1000, Kg=1-Kr-Kb; derive r,b then g from luminance equation; integer Y translation of exact rounded chroma offsets','rounding':'floor(value+1/2), saturated to [0,255]','pydicom_float32_disagreeing_pixels':differences,'examples':examples,'pydicom_runtime_warnings':sorted(warning_messages)}
path.with_suffix('.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
