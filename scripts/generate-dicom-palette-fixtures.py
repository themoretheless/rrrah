"""CC0 authored classic DICOM palettes. Pydicom indices, independent NumPy LUT.
Palette storage is parsed/verified separately from authored component arrays.
"""
from pathlib import Path
import json
import numpy as np
import pydicom
from pydicom.dataset import FileDataset,FileMetaDataset
from pydicom.uid import ExplicitVRLittleEndian,ExplicitVRBigEndian,ImplicitVRLittleEndian,SecondaryCaptureImageStorage
root=Path(__file__).resolve().parents[1]/'tests/fixtures/raster';manifest=['file\twidth\theight\tframes\tbits\tprofile_bytes\tlabel'];notes=[]
for enc in ('explicit','implicit','big-endian'):
 for variant in ('packed8','legacy8','odd8','signed12','maximum16','icc16','srgb8'):
  big=enc=='big-endian';implicit=enc=='implicit';signed=variant in ('signed12','icc16');bits=16 if signed or variant=='maximum16' else 8;depth=16 if bits==16 else 8
  entries=65536 if variant=='maximum16' else 5 if variant=='odd8' else 6;first=-2 if signed else 0 if variant=='maximum16' else 2;frames=1 if variant=='maximum16' else 2 if signed else 3;rows=2 if bits==16 else 1
  indices=np.array([-2048,-3,-2,-1,0,1,2,3,4,2047,-1,-2] if signed else [0,1,32767,32768,65534,65535] if variant=='maximum16' else [0,1,2,3,6,7,8,9,255],dtype=np.int16 if signed else np.uint16 if bits==16 else np.uint8).reshape(frames,rows,3)
  cols=3
  if variant=='maximum16':
   rows=cols=256;indices=np.arange(65536,dtype=np.uint16).reshape(1,rows,cols)
  n=np.arange(entries,dtype=np.uint32);maximum=(1<<depth)-1
  tables=[(n*17%(maximum+1)).astype(np.uint16),(maximum-n%(maximum+1)).astype(np.uint16),(n*257%(maximum+1)).astype(np.uint16)]
  name=f'dicom-palette-{enc}-{variant}.dcm';meta=FileMetaDataset();meta.FileMetaInformationVersion=b'\0\1';meta.MediaStorageSOPClassUID=SecondaryCaptureImageStorage;meta.MediaStorageSOPInstanceUID=f'1.2.826.0.1.3680043.10.543.95{len(manifest)}'
  meta.ImplementationClassUID='1.2.826.0.1.3680043.10.543.999';meta.TransferSyntaxUID=ImplicitVRLittleEndian if implicit else ExplicitVRBigEndian if big else ExplicitVRLittleEndian
  ds=FileDataset(str(root/name),{},file_meta=meta,preamble=bytes(128));ds.SOPClassUID=meta.MediaStorageSOPClassUID;ds.SOPInstanceUID=meta.MediaStorageSOPInstanceUID
  ds.Rows=rows;ds.Columns=cols;ds.NumberOfFrames=str(frames);ds.SamplesPerPixel=1;ds.PhotometricInterpretation='PALETTE COLOR';ds.BitsAllocated=bits;ds.BitsStored=12 if signed else bits;ds.HighBit=ds.BitsStored-1;ds.PixelRepresentation=int(signed)
  for channel,values in zip(('Red','Green','Blue'),tables):
   ds.add_new((0x28,{'Red':0x1101,'Green':0x1102,'Blue':0x1103}[channel]),'SS' if signed else 'US',[0 if entries==65536 else entries,first,depth])
   if depth==16 or variant=='legacy8':data=values.astype('>u2' if big else '<u2').tobytes()
   else:
    data=values.astype(np.uint8).tobytes()+bytes(entries%2)
    if big:data=b''.join(data[i:i+2][::-1] for i in range(0,len(data),2))
   ds.add_new((0x28,{'Red':0x1201,'Green':0x1202,'Blue':0x1203}[channel]),'OW',data)
  if bits==16:
   words=(indices.astype(np.int32)&((1<<ds.BitsStored)-1)).astype('>u2' if big else '<u2')
   if signed:words|=0xa000
   data=words.tobytes()
  else:
   data=indices.tobytes()+bytes(indices.size%2)
   if big:data=b''.join(data[i:i+2][::-1] for i in range(0,len(data),2))
  ds.PixelData=data;ds['PixelData'].VR='OW' if bits==16 or big or implicit else 'OB'
  if variant=='icc16':ds.ICCProfile=(root/'dicom-rgb-profile.icc').read_bytes()
  if variant=='srgb8':ds.ColorSpace='SRGB'
  pydicom.dcmwrite(root/name,ds,little_endian=not big,implicit_vr=implicit,enforce_file_format=True)
  restored=pydicom.dcmread(root/name);raw=restored.pixel_array.reshape(indices.shape);assert np.array_equal(raw,indices)
  for channel,expected in zip(('Red','Green','Blue'),tables):
   data=getattr(restored,channel+'PaletteColorLookupTableData')
   if depth==16 or variant=='legacy8':actual=np.frombuffer(data,dtype='>u2' if big else '<u2')
   else:
    if big:data=b''.join(data[i:i+2][::-1] for i in range(0,len(data),2))
    actual=np.frombuffer(data[:entries],dtype=np.uint8)
   assert np.array_equal(actual,expected)
  for i in range(frames):
   mapped=np.clip(raw[i].astype(np.int64)-first,0,entries-1)
   rgba=np.stack([tables[0][mapped],tables[1][mapped],tables[2][mapped],np.full_like(mapped,maximum)],axis=-1).astype('<u2' if depth==16 else np.uint8)
   (root/f'{name}.{i}.rgba{depth}').write_bytes(rgba.tobytes())
  manifest.append(f'{name}\t{cols}\t{rows}\t{frames}\t{depth}\t{588 if variant=="icc16" else 0}\t{"SRGB" if variant=="srgb8" else "-"}')
(root/'dicom-palette-manifest.tsv').write_text('\n'.join(manifest)+'\n');print('wrote',len(manifest)-1,'palette containers')
