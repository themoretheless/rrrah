"""CC0 authored YBR encodings: pydicom raw unpack + exact Fraction RGB oracle."""
from pathlib import Path
from fractions import Fraction
import json
import numpy as np
import pydicom
from pydicom.pixels import pixel_array,convert_color_space
from pydicom.dataset import FileDataset,FileMetaDataset
from pydicom.uid import ExplicitVRLittleEndian,ExplicitVRBigEndian,ImplicitVRLittleEndian,SecondaryCaptureImageStorage
root=Path(__file__).resolve().parents[1]/'tests/fixtures/raster'
kr,kb=Fraction('0.299'),Fraction('0.114');kg=1-kr-kb

def rgb(values):
 y,cb,cr=map(int,values);cb-=128;cr-=128
 r=y+2*(1-kr)*cr;b=y+2*(1-kb)*cb;g=(y-kr*r-kb*b)/kg
 return [max(0,min(255,(2*x.numerator+x.denominator)//(2*x.denominator))) for x in (r,g,b)]

cases=[]
for enc in ('explicit','implicit','big-endian'):
 for planar in (0,1):cases.append((enc,'YBR_FULL',planar,'plain'))
 cases.extend([(enc,'YBR_FULL_422',0,'plain'),(enc,'YBR_FULL',0,'icc'),(enc,'YBR_FULL_422',0,'srgb')])
manifest=['file\twidth\theight\tframes\tbits\tprofile_bytes\tlabel'];differences=[]
for i,(enc,photo,planar,variant) in enumerate(cases):
 big=enc=='big-endian';implicit=enc=='implicit';packed=photo=='YBR_FULL_422';frames=2 if packed else 3;rows=2 if packed else 1;cols=4 if packed else 3
 name=f'dicom-ybr-{enc}-{"422" if packed else "full"}-planar{planar}-{variant}.dcm'
 if packed:
  pairs=np.array([[(p*53)%256,(p*71+19)%256,(p*83)%256,(p*11+178)%256] for p in range(frames*rows*cols//2)],dtype=np.uint8)
  pairs[0]=[19,20,78,178]
  values=np.empty((len(pairs)*2,3),dtype=np.uint8);values[0::2]=pairs[:,[0,2,3]];values[1::2]=pairs[:,[1,2,3]]
  values=values.reshape(frames,rows,cols,3);data=pairs.tobytes()
 else:
  values=np.array([[(p*37+19)%256,(p*73+78)%256,(p*31+178)%256] for p in range(frames*rows*cols)],dtype=np.uint8).reshape(frames,rows,cols,3)
  values[0,0,0]=[19,78,178];values[0,0,1]=[0,0,0];values[0,0,2]=[255,255,255]
  data=(values if planar==0 else values.transpose(0,3,1,2)).tobytes()
 meta=FileMetaDataset();meta.FileMetaInformationVersion=b'\0\1';meta.MediaStorageSOPClassUID=SecondaryCaptureImageStorage;meta.MediaStorageSOPInstanceUID=f'1.2.826.0.1.3680043.10.543.93{i}'
 meta.ImplementationClassUID='1.2.826.0.1.3680043.10.543.999';meta.TransferSyntaxUID=ImplicitVRLittleEndian if implicit else ExplicitVRBigEndian if big else ExplicitVRLittleEndian
 ds=FileDataset(str(root/name),{},file_meta=meta,preamble=bytes(128));ds.SOPClassUID=meta.MediaStorageSOPClassUID;ds.SOPInstanceUID=meta.MediaStorageSOPInstanceUID
 ds.Rows=rows;ds.Columns=cols;ds.SamplesPerPixel=3;ds.PhotometricInterpretation=photo;ds.PlanarConfiguration=planar;ds.NumberOfFrames=str(frames);ds.BitsAllocated=8;ds.BitsStored=8;ds.HighBit=7;ds.PixelRepresentation=0
 vr='OW' if big or implicit else 'OB'
 if big:
  data+=bytes(len(data)%2);data=b''.join(data[j:j+2][::-1] for j in range(0,len(data),2))
 ds.PixelData=data;ds['PixelData'].VR=vr
 if variant=='icc':ds.ICCProfile=(root/'dicom-rgb-profile.icc').read_bytes()
 if variant=='srgb':ds.ColorSpace='SRGB'
 pydicom.dcmwrite(root/name,ds,little_endian=not big,implicit_vr=implicit,enforce_file_format=True)
 decoded=pydicom.dcmread(root/name);raw=pixel_array(decoded,raw=True).reshape(values.shape)
 assert np.array_equal(raw,values),(name,raw.tolist(),values.tolist())
 for frame in range(frames):
  exact=np.array([rgb(p) for p in raw[frame].reshape(-1,3)],dtype=np.uint8).reshape(rows,cols,3)
  reference=convert_color_space(raw[frame],'YBR_FULL','RGB');bad=np.any(exact!=reference,axis=2)
  for row,col in np.argwhere(bad):differences.append({'file':name,'frame':frame,'pixel':[int(col),int(row)],'source':raw[frame,row,col].tolist(),'exact':exact[row,col].tolist(),'pydicom':reference[row,col].tolist()})
  rgba=np.concatenate([exact,np.full((rows,cols,1),255,dtype=np.uint8)],axis=2)
  (root/f'{name}.{frame}.rgba8').write_bytes(rgba.tobytes())
 manifest.append(f'{name}\t{cols}\t{rows}\t{frames}\t8\t{588 if variant=="icc" else 0}\t{"SRGB" if variant=="srgb" else "-"}')
(root/'dicom-ybr-manifest.tsv').write_text('\n'.join(manifest)+'\n')
(root/'dicom-ybr-rounding-differences.json').write_text(json.dumps({'scope':'exact Fraction versus pydicom float32 RGB conversion; pydicom raw source unpack matches every sample','differences':differences},indent=2)+'\n')
print('wrote',len(cases),'YBR containers;',len(differences),'float32 rounding disagreements retained')
