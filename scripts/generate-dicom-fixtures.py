"""CC0 authored native scalar pixels; independent pydicom 3.0.2 pixel/rescale oracles.
Run with Python, numpy 2.2.6 and pydicom 3.0.2. No Rrrah decoder is used.
These test pixel encoding, not complete clinical IOD conformance.
"""
from pathlib import Path
import numpy as np
import pydicom
from pydicom.dataset import FileDataset, FileMetaDataset
from pydicom.pixels import apply_modality_lut
from pydicom.uid import ExplicitVRLittleEndian, CTImageStorage
root=Path(__file__).resolve().parents[1]/'tests/fixtures/raster'
cases=[
 ('dicom-signed12-rescale.dcm',16,12,1,'MONOCHROME2',2,[-2048,-1,0,1,1024,2047,2047,1024,1,0,-1,-2048],2,-1000,-5096,3094),
 ('dicom-unsigned16.dcm',16,16,0,'MONOCHROME2',1,[0,1,32767,32768,65534,65535],None,None,0,65535),
 ('dicom-unsigned16-tiff-preamble.dcm',16,16,0,'MONOCHROME2',1,[0,1,32767,32768,65534,65535],None,None,0,65535),
 ('dicom-mono1-odd8.dcm',8,8,0,'MONOCHROME1',1,[0,127,255],None,None,0,255),
]
manifest=['file\twidth\theight\tframes\tminimum\tmaximum']
for index,(name,bits,stored,signed,photo,frames,values,slope,intercept,lo,hi) in enumerate(cases):
 meta=FileMetaDataset();meta.FileMetaInformationVersion=b'\0\1'
 meta.MediaStorageSOPClassUID=CTImageStorage
 meta.MediaStorageSOPInstanceUID=f'1.2.826.0.1.3680043.10.543.900{index}'
 meta.TransferSyntaxUID=ExplicitVRLittleEndian;meta.ImplementationClassUID='1.2.826.0.1.3680043.10.543.999'
 ds=FileDataset(str(root/name),{},file_meta=meta,preamble=(b'II*\0'+bytes(124)) if 'tiff-preamble' in name else bytes(128))
 ds.SOPClassUID=meta.MediaStorageSOPClassUID;ds.SOPInstanceUID=meta.MediaStorageSOPInstanceUID
 ds.PatientName='RRRAH^SYNTHETIC';ds.PatientID='SYNTHETIC';ds.Modality='CT'
 ds.Rows=1 if bits==8 else 2;ds.Columns=3;ds.SamplesPerPixel=1;ds.PhotometricInterpretation=photo
 ds.NumberOfFrames=str(frames);ds.BitsAllocated=bits;ds.BitsStored=stored;ds.HighBit=stored-1;ds.PixelRepresentation=signed
 if slope is not None:ds.RescaleSlope=str(slope);ds.RescaleIntercept=str(intercept)
 if bits==16:
  words=np.array([v & ((1<<stored)-1) for v in values],dtype='<u2')
  if stored<16:words|=0xa000 # exercise unused high bits, not presumed zero
  ds.PixelData=words.tobytes()
 else:ds.PixelData=bytes(values)
 ds.save_as(root/name,enforce_file_format=True)
 decoded=pydicom.dcmread(root/name)
 raw=decoded.pixel_array.reshape(frames,ds.Rows,ds.Columns)
 assert raw.reshape(-1).tolist()==values
 physical=apply_modality_lut(raw,decoded).astype(np.float64)
 for frame in range(frames):
  gray=physical[frame].reshape(-1)
  rgba=np.stack([gray,gray,gray,np.ones_like(gray)],axis=1).astype('<f4')
  (root/f'{name}.{frame}.raw-f32').write_bytes(rgba.tobytes())
  window=np.clip((gray-lo)/(hi-lo),0,1)
  if photo=='MONOCHROME1':window=1-window
  (root/f'{name}.{frame}.window-f32').write_bytes(np.stack([window,window,window,np.ones_like(window)],axis=1).astype('<f4').tobytes())
 manifest.append(f'{name}\t{ds.Columns}\t{ds.Rows}\t{frames}\t{lo}\t{hi}')
(root/'dicom-manifest.tsv').write_text('\n'.join(manifest)+'\n')
print('wrote 4 scalar containers and pydicom raw/rescale oracles',pydicom.__version__)
