"""Independent full packaged palette oracle; source read in place (not copied)."""
from pathlib import Path
import hashlib,json,sys
import numpy as np
import pydicom
from pydicom.pixels import apply_color_lut
source,target=map(Path,sys.argv[1:3]);target.mkdir(parents=True,exist_ok=True)
ds=pydicom.dcmread(source);assert ds.PhotometricInterpretation=='PALETTE COLOR'
color=apply_color_lut(ds.pixel_array,ds);assert color.shape==(350,800,3) and color.dtype==np.uint16
rgba=np.concatenate([color,np.full((350,800,1),65535,dtype=np.uint16)],axis=2).astype('<u2').tobytes();(target/'rgba16.bin').write_bytes(rgba)
report={'source':str(source),'source_bytes':source.stat().st_size,'source_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),'dimensions':[800,350],'pixels':280000,'oracle_sha256':hashlib.sha256(rgba).hexdigest(),'oracle_bytes':len(rgba),'pydicom':pydicom.__version__,'numpy':np.__version__,'root_sequences':[str(el.tag) for el in ds if el.VR=='SQ']}
(target/'result.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
