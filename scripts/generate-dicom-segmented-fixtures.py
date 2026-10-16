"""CC0 segmented16 controls: pydicom indices + independent Fraction segment VM.
Indirect offsets are bytes, even for16-bit streams. No indirect-to-indirect copy.
"""
from pathlib import Path
from fractions import Fraction
import json,struct
import numpy as np
import pydicom
from pydicom.pixels.processing import _expand_segmented_lut
root=Path(__file__).resolve().parents[1]/'tests/fixtures/raster';manifest=['file\twidth\theight\tframes\tbits\tprofile_bytes\tlabel\taspect\tscratch_bytes'];differences=[]
def expand(data,big):
 words=struct.unpack(('>' if big else '<')+str(len(data)//2)+'H',data);segments={};at=0
 while at<len(words):
  start=at;op,n=words[at:at+2];at+=2
  size=n if op==0 else 1 if op==1 else 2 if op==2 else None;assert size is not None
  segments[start*2]=(op,n,words[at:at+size],(at+size)*2);at+=size
 out=[]
 def emit(op,n,values):
  if op==0:out.extend(values)
  else:
   assert op==1 and out and n;first=out[-1];last=values[0]
   out.extend(round(Fraction((n-i)*first+i*last,n)) for i in range(1,n+1))
 for start,(op,n,values,end) in segments.items():
  if op!=2:emit(op,n,values)
  else:
   assert out;target=values[0]+(values[1]<<16)
   for _ in range(n):
    op2,n2,v2,target=segments[target];assert op2!=2;emit(op2,n2,v2)
 return np.array(out,dtype=np.uint16)
for enc in ('explicit','implicit','big-endian'):
 for variant in ('linear','indirect-zero','indirect-discrete','full-ramp','icc-indirect'):
  base='maximum16' if variant=='full-ramp' else 'icc16' if variant=='icc-indirect' else 'signed12' if variant=='indirect-discrete' else 'packed8'
  ds=pydicom.dcmread(root/f'dicom-palette-{enc}-{base}.dcm');big=enc=='big-endian';implicit=enc=='implicit'
  for tag in (0x00281201,0x00281202,0x00281203):del ds[tag]
  if variant=='linear':streams=[[0,1,0,1,4,5],[0,1,100,1,4,0],[0,1,65535,1,4,65530]]
  elif variant=='indirect-zero':streams=[[0,1,v,1,2,v+5,0,1,0,2,1,6,0] for v in (0,100,1000)]
  elif variant=='full-ramp':streams=[[0,1,a,1,65535,b] for a,b in [(0,65535),(65535,0),(10000,60000)]]
  else:streams=[[0,3,*v,2,1,0,0] for v in [(3,100,65535),(7,200,30000),(65535,12345,0)]]
  tables=[]
  for c,words in enumerate(streams):
   data=struct.pack(('>' if big else '<')+str(len(words))+'H',*words);table=expand(data,big);tables.append(table)
   ds.add_new((0x28,0x1221+c),'OW',data);descriptor=ds[(0x28,0x1101+c)].value;descriptor[0]=0 if len(table)==65536 else len(table);descriptor[2]=16
   try:
    ref=_expand_segmented_lut(tuple(words),'>H' if big else '<H')
    if not np.array_equal(table,np.array(ref,dtype=np.uint16)):differences.append({'encoding':enc,'variant':variant,'channel':c,'native_spec_entries':len(table),'pydicom_entries':len(ref),'scope':'pydicom helper uses byte offset as16bit tuple index; native independent VM uses actualbyteoffset'})
   except (ValueError,IndexError) as e:differences.append({'encoding':enc,'variant':variant,'channel':c,'pydicom_error':type(e).__name__})
  name=f'dicom-segmented-{enc}-{variant}.dcm';ds.SOPInstanceUID=ds.file_meta.MediaStorageSOPInstanceUID=f'1.2.826.0.1.3680043.10.543.96{len(manifest)}'
  pydicom.dcmwrite(root/name,ds,little_endian=not big,implicit_vr=implicit,enforce_file_format=True)
  restored=pydicom.dcmread(root/name);raw=restored.pixel_array.reshape(int(ds.NumberOfFrames),ds.Rows,ds.Columns);entries=len(tables[0]);first=ds.RedPaletteColorLookupTableDescriptor[1]
  for c in range(3):assert np.array_equal(expand(restored[(0x28,0x1221+c)].value,big),tables[c])
  for i in range(int(ds.NumberOfFrames)):
   mapped=np.clip(raw[i].astype(np.int64)-first,0,entries-1);rgba=np.stack([tables[0][mapped],tables[1][mapped],tables[2][mapped],np.full_like(mapped,65535)],axis=-1).astype('<u2');(root/f'{name}.{i}.rgba16').write_bytes(rgba.tobytes())
  profile=len(ds.get('ICCProfile',b''));manifest.append(f'{name}\t{ds.Columns}\t{ds.Rows}\t{ds.NumberOfFrames}\t16\t{profile}\t-\t-\t{entries*6}')
(root/'dicom-segmented-manifest.tsv').write_text('\n'.join(manifest)+'\n');(root/'dicom-segmented-oracle-differences.json').write_text(json.dumps(differences,indent=2)+'\n');print('wrote',len(manifest)-1,'segmented16containers;',len(differences),'pydicom helper disagreements retained')
