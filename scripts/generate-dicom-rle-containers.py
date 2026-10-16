"""CC0 RLE containers from authored sources. Independent pydicom pixel checks.
Keeps color/window oracle bytes from proven source encodings, not Rrrah output.
"""
from pathlib import Path
import numpy as np
import pydicom
from pydicom.pixels import pixel_array,apply_modality_lut
from pydicom.uid import RLELossless
root=Path(__file__).resolve().parents[1]/'tests/fixtures/raster'
color=['file\twidth\theight\tframes\tbits\tprofile_bytes\tlabel\taspect\tscratch_bytes']
scalar=['file\twidth\theight\tframes\tminimum\tmaximum\taspect\tscratch_bytes']
for family,source in [('scalar','dicom-signed12-rescale.dcm')]+[('color',s) for s in [
 'dicom-rgb-explicit-8-planar0-plain.dcm','dicom-rgb-explicit-8-planar1-plain.dcm',
 'dicom-rgb-explicit-16-planar0-plain.dcm','dicom-rgb-explicit-16-planar1-plain.dcm',
 'dicom-rgb-explicit-16-planar1-icc.dcm','dicom-ybr-explicit-full-planar0-plain.dcm',
 'dicom-ybr-explicit-full-planar1-plain.dcm','dicom-palette-explicit-packed8.dcm',
 'dicom-palette-explicit-signed12.dcm','dicom-segmented-explicit-indirect-zero.dcm',
 'dicom-segmented-explicit-full-ramp.dcm']]:
 ds=pydicom.dcmread(root/source);raw=pixel_array(ds,raw=True).copy();frames=int(ds.get('NumberOfFrames',1))
 if ds.PhotometricInterpretation=='PALETTE COLOR' and ds.PixelRepresentation:
  raw=(raw.astype(np.int32)+2048).astype(np.uint16)
  ds.PixelRepresentation=0;ds.BitsStored=16;ds.HighBit=15
  for c in (1,2,3):
   descriptor=ds[(0x28,0x1100+c)];descriptor.value[1]+=2048;descriptor.VR='US'
 name='dicom-rle-'+source.removeprefix('dicom-');ds.SOPInstanceUID=ds.file_meta.MediaStorageSOPInstanceUID=f'1.2.826.0.1.3680043.10.543.97{len(color)+len(scalar)}'
 ds.compress(RLELossless,arr=raw,encoding_plugin='pydicom',generate_instance_uid=False)
 pydicom.dcmwrite(root/name,ds,enforce_file_format=True)
 restored=pydicom.dcmread(root/name);decoded=pixel_array(restored,raw=True);assert np.array_equal(raw,decoded)
 size=int(ds.Rows)*int(ds.Columns)*int(ds.SamplesPerPixel)*int(ds.BitsAllocated)//8;scratch=size+size%2
 if 'SegmentedRedPaletteColorLookupTableData' in ds:scratch+=(int(ds.RedPaletteColorLookupTableDescriptor[0]) or 65536)*6
 if family=='scalar':
  physical=apply_modality_lut(decoded,restored).astype(np.float64).reshape(frames,ds.Rows,ds.Columns);lo,hi=-5096.,3094.
  for i in range(frames):
   gray=physical[i];rgba=np.stack([gray,gray,gray,np.ones_like(gray)],axis=-1).astype('<f4');(root/f'{name}.{i}.raw-f32').write_bytes(rgba.tobytes());assert rgba.tobytes()==(root/f'{source}.{i}.raw-f32').read_bytes()
   for label,a,b in [('window',lo,hi),('alternate-window',lo+(hi-lo)/4,lo+3*(hi-lo)/4)]:
    g=np.clip((gray-a)/(b-a),0,1);(root/f'{name}.{i}.{label}-f32').write_bytes(np.stack([g,g,g,np.ones_like(g)],axis=-1).astype('<f4').tobytes())
  scalar.append(f'{name}\t{ds.Columns}\t{ds.Rows}\t{frames}\t{lo}\t{hi}\t-\t{scratch}')
 else:
  depth=int(ds.RedPaletteColorLookupTableDescriptor[2]) if ds.PhotometricInterpretation=='PALETTE COLOR' else int(ds.BitsAllocated)
  for i in range(frames):(root/f'{name}.{i}.rgba{depth}').write_bytes((root/f'{source}.{i}.rgba{depth}').read_bytes())
  color.append(f'{name}\t{ds.Columns}\t{ds.Rows}\t{frames}\t{depth}\t{len(ds.get("ICCProfile",b""))}\t{ds.get("ColorSpace","-")}\t-\t{scratch}')
(root/'dicom-rle-color-manifest.tsv').write_text('\n'.join(color)+'\n');(root/'dicom-rle-scalar-manifest.tsv').write_text('\n'.join(scalar)+'\n');print('wrote',len(color)-1,'color and',len(scalar)-1,'scalar RLEcontainers; states',sum(int(s.split('\t')[3]) for s in color[1:])+sum(int(s.split('\t')[3])*2 for s in scalar[1:]))
