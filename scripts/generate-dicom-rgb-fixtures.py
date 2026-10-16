"""CC0 authored RGB DICOM encodings; pydicom3.0.2 independent sample oracle.
ICC bytes are retained from the existing pattern.profiled.png fixture.
"""
from pathlib import Path
import struct,zlib
import numpy as np
import pydicom
from pydicom.dataset import FileDataset,FileMetaDataset
from pydicom.uid import ExplicitVRLittleEndian,ExplicitVRBigEndian,ImplicitVRLittleEndian,SecondaryCaptureImageStorage
root=Path(__file__).resolve().parents[1]/'tests/fixtures/raster'
png=(root/'pattern.profiled.png').read_bytes();at=8;icc=None
while at<len(png):
 n=struct.unpack('>I',png[at:at+4])[0];kind=png[at+4:at+8];data=png[at+8:at+8+n];at+=12+n
 if kind==b'iCCP':
  split=data.index(0);assert data[split+1]==0;icc=zlib.decompress(data[split+2:])
assert icc and icc[16:20]==b'RGB '
(root/'dicom-rgb-profile.icc').write_bytes(icc)
cases=[]
for encoding in ('explicit','implicit','big-endian'):
 for bits in (8,16):
  for planar in (0,1):cases.append((encoding,bits,planar,'plain'))
  cases.append((encoding,bits,1,'icc'))
 cases.append((encoding,8,0,'srgb'))
for planar in (0,1):cases.append(('big-endian',8,planar,'ow'))
manifest=['file\twidth\theight\tframes\tbits\tprofile_bytes\tlabel']
for i,(encoding,bits,planar,variant) in enumerate(cases):
 name=f'dicom-rgb-{encoding}-{bits}-planar{planar}-{variant}.dcm'
 big=encoding=='big-endian';implicit=encoding=='implicit';frames=3 if bits==8 else 2;rows=1 if bits==8 else 2
 values=np.array([(j*37+j*j*11)%(1<<bits) for j in range(frames*rows*3*3)],dtype=np.uint16 if bits==16 else np.uint8).reshape(frames,rows,3,3)
 values[0,0,0]=[0,(1<<bits)//2,(1<<bits)-1]
 meta=FileMetaDataset();meta.FileMetaInformationVersion=b'\0\1';meta.MediaStorageSOPClassUID=SecondaryCaptureImageStorage;meta.MediaStorageSOPInstanceUID=f'1.2.826.0.1.3680043.10.543.92{i}'
 meta.ImplementationClassUID='1.2.826.0.1.3680043.10.543.999';meta.TransferSyntaxUID=ImplicitVRLittleEndian if implicit else ExplicitVRBigEndian if big else ExplicitVRLittleEndian
 ds=FileDataset(str(root/name),{},file_meta=meta,preamble=bytes(128));ds.SOPClassUID=meta.MediaStorageSOPClassUID;ds.SOPInstanceUID=meta.MediaStorageSOPInstanceUID
 ds.Rows=rows;ds.Columns=3;ds.SamplesPerPixel=3;ds.PhotometricInterpretation='RGB';ds.PlanarConfiguration=planar;ds.NumberOfFrames=str(frames)
 ds.BitsAllocated=bits;ds.BitsStored=bits;ds.HighBit=bits-1;ds.PixelRepresentation=0
 layout=values if planar==0 else values.transpose(0,3,1,2)
 data=layout.astype(('>' if big else '<')+('u2' if bits==16 else 'u1')).tobytes()
 vr='OW' if bits==16 or implicit or variant=='ow' else 'OB'
 if big and bits==8 and vr=='OW':
  data+=bytes(len(data)%2);data=b''.join(data[j:j+2][::-1] for j in range(0,len(data),2))
 ds.PixelData=data;ds['PixelData'].VR=vr
 if variant=='icc':ds.ICCProfile=icc
 if variant=='srgb':ds.ColorSpace='SRGB'
 pydicom.dcmwrite(root/name,ds,little_endian=not big,implicit_vr=implicit,enforce_file_format=True)
 decoded=pydicom.dcmread(root/name);a=decoded.pixel_array.reshape(values.shape);assert np.array_equal(a,values)
 for frame in range(frames):
  alpha=np.full((*a[frame].shape[:2],1),(1<<bits)-1,dtype=a.dtype)
  rgba=np.concatenate([a[frame],alpha],axis=2).astype('<u2' if bits==16 else 'u1')
  (root/f'{name}.{frame}.rgba{bits}').write_bytes(rgba.tobytes())
 manifest.append(f'{name}\t3\t{rows}\t{frames}\t{bits}\t{len(icc) if variant=="icc" else 0}\t{"SRGB" if variant=="srgb" else "-"}')
(root/'dicom-rgb-manifest.tsv').write_text('\n'.join(manifest)+'\n')
print('wrote',len(cases),'RGB containers;',len(icc),'ICC bytes')
