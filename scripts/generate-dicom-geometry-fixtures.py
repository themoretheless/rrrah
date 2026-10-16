"""CC0 authored DICOM geometry controls; pydicom pixels/metadata oracle.
No image resampling; existing scalar/RGB source pixels are kept exactly.
"""
from pathlib import Path
import numpy as np
import pydicom
from pydicom.pixels import apply_modality_lut
root=Path(__file__).resolve().parents[1]/'tests/fixtures/raster'
color=['file\twidth\theight\tframes\tbits\tprofile_bytes\tlabel\taspect']
scalar=['file\twidth\theight\tframes\tminimum\tmaximum\taspect']
for enc in ('explicit','implicit','big-endian'):
 for kind in ('color','scalar'):
  source=f'dicom-rgb-{enc}-8-planar0-plain.dcm' if kind=='color' else {'explicit':'dicom-signed12-rescale.dcm','implicit':'dicom-implicit-signed12-rescale.dcm','big-endian':'dicom-big-endian-signed12-rescale.dcm'}[enc]
  for field,values in [('PixelSpacing',['2','3']),('PixelAspectRatio',[3,2])]+([('ImagerPixelSpacing',['4','3']),('NominalScannedPixelSpacing',['.75','1.5'])] if kind=='color' else []):
   name=f'dicom-geometry-{enc}-{kind}-{field}.dcm';ds=pydicom.dcmread(root/source)
   ds.SOPInstanceUID=ds.file_meta.MediaStorageSOPInstanceUID=f'1.2.826.0.1.3680043.10.543.94{len(color)+len(scalar)}'
   setattr(ds,field,values)
   pydicom.dcmwrite(root/name,ds,little_endian=enc!='big-endian',implicit_vr=enc=='implicit',enforce_file_format=True)
   decoded=pydicom.dcmread(root/name);pair=getattr(decoded,field);aspect=np.float32(float(pair[1])/float(pair[0]))
   raw=decoded.pixel_array;frames=int(ds.NumberOfFrames)
   if kind=='color':
    raw=raw.reshape(frames,ds.Rows,ds.Columns,3)
    for i in range(frames):
     rgba=np.concatenate([raw[i],np.full((ds.Rows,ds.Columns,1),255,dtype=np.uint8)],axis=2)
     (root/f'{name}.{i}.rgba8').write_bytes(rgba.tobytes())
     assert rgba.tobytes()==(root/f'{source}.{i}.rgba8').read_bytes()
    color.append(f'{name}\t{ds.Columns}\t{ds.Rows}\t{frames}\t8\t0\t-\t{float(aspect)}')
   else:
    physical=apply_modality_lut(raw,decoded).astype(np.float64).reshape(frames,ds.Rows,ds.Columns)
    lo,hi=-5096.,3094.
    for i in range(frames):
     gray=physical[i];rgba=np.stack([gray,gray,gray,np.ones_like(gray)],axis=-1).astype('<f4')
     (root/f'{name}.{i}.raw-f32').write_bytes(rgba.tobytes());assert rgba.tobytes()==(root/f'{source}.{i}.raw-f32').read_bytes()
     for label,a,b in [('window',lo,hi),('alternate-window',lo+(hi-lo)/4,lo+3*(hi-lo)/4)]:
      g=np.clip((gray-a)/(b-a),0,1);rgba=np.stack([g,g,g,np.ones_like(g)],axis=-1).astype('<f4');(root/f'{name}.{i}.{label}-f32').write_bytes(rgba.tobytes())
    scalar.append(f'{name}\t{ds.Columns}\t{ds.Rows}\t{frames}\t{lo}\t{hi}\t{float(aspect)}')
(root/'dicom-geometry-color-manifest.tsv').write_text('\n'.join(color)+'\n')
(root/'dicom-geometry-scalar-manifest.tsv').write_text('\n'.join(scalar)+'\n')
print('wrote',len(color)-1,'color and',len(scalar)-1,'scalar geometry containers')
