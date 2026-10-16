"""Full pixel pydicom3.0.2 oracle for packaged RGB controls, sources read in place."""
from pathlib import Path
import hashlib,json,sys
from fractions import Fraction
import numpy as np
import pydicom
source,target=map(Path,sys.argv[1:3]);target.mkdir(parents=True,exist_ok=True)
manifest=[];results=[]
for name in ['ExplVR_BigEnd.dcm','examples_rgb_color.dcm','SC_rgb_small_odd.dcm','SC_rgb_small_odd_big_endian.dcm','SC_ybr_full_422_uncompressed.dcm']:
 path=source/name;ds=pydicom.dcmread(path);assert ds.BitsAllocated==8 and ds.PhotometricInterpretation in ('RGB','YBR_FULL_422')
 a=ds.pixel_array
 differences=0
 if ds.PhotometricInterpretation!='RGB':
  raw=pydicom.pixels.pixel_array(ds,raw=True);kr,kb=Fraction('0.299'),Fraction('0.114');kg=1-kr-kb
  def convert(pixel):
   y,cb,cr=map(int,pixel);r=y+2*(1-kr)*(cr-128);b=y+2*(1-kb)*(cb-128);g=(y-kr*r-kb*b)/kg
   return [max(0,min(255,(2*v.numerator+v.denominator)//(2*v.denominator))) for v in (r,g,b)]
  exact=np.array([convert(p) for p in raw.reshape(-1,3)],dtype=np.uint8).reshape(a.shape)
  differences=int(np.count_nonzero(np.any(exact!=a,axis=-1)));a=exact
 assert a.shape==(ds.Rows,ds.Columns,3)
 rgba=np.concatenate([a,np.full((ds.Rows,ds.Columns,1),255,dtype=np.uint8)],axis=2).tobytes()
 (target/f'{name}.rgba8').write_bytes(rgba)
 manifest.append(f'{name}\t{ds.Columns}\t{ds.Rows}')
 results.append({'name':name,'source_sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'source_bytes':path.stat().st_size,'dimensions':[int(ds.Columns),int(ds.Rows)],'pixel_count':int(ds.Rows*ds.Columns),'planar':int(ds.PlanarConfiguration),'photometric':ds.PhotometricInterpretation,'pydicom_conversion_differences':differences,'pixel_vr':ds['PixelData'].VR,'syntax':str(ds.file_meta.TransferSyntaxUID),'oracle_sha256':hashlib.sha256(rgba).hexdigest()})
(target/'manifest.tsv').write_text('\n'.join(manifest)+'\n');(target/'result.json').write_text(json.dumps({'pydicom':pydicom.__version__,'sources':results},indent=2)+'\n');print(json.dumps(results))
