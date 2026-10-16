"""Extract packaged RLE frame controls into a temporary oracle directory.
Sources read in place; pydicom pure Python decodes every component independently.
"""
from pathlib import Path
import hashlib,json,sys
import numpy as np
import pydicom
from pydicom.encaps import generate_frames
source,target=map(Path,sys.argv[1:3]);target.mkdir(parents=True,exist_ok=True);manifest=[];results=[]
for name in ['MR_small_RLE.dcm','SC_rgb_rle.dcm','SC_rgb_rle_2frame.dcm','SC_rgb_rle_16bit.dcm','SC_rgb_rle_16bit_2frame.dcm']:
 path=source/name;ds=pydicom.dcmread(path);ds.pixel_array_options(decoding_plugin='pydicom');raw=ds.pixel_array
 frames=int(ds.get('NumberOfFrames',1));samples=int(ds.SamplesPerPixel);bits=int(ds.BitsAllocated)
 raw=raw.reshape((frames,int(ds.Rows),int(ds.Columns),samples) if samples>1 else (frames,int(ds.Rows),int(ds.Columns)))
 encoded=list(generate_frames(ds.PixelData,number_of_frames=frames));assert len(encoded)==frames
 item={'source':str(path),'sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'frames':[]}
 for i,data in enumerate(encoded):
  key=f'{name}.{i}';dtype=('<i2' if ds.PixelRepresentation else '<u2') if bits==16 else np.uint8;pixels=raw[i].astype(dtype).tobytes()
  (target/f'{key}.rle').write_bytes(data);(target/f'{key}.pixels').write_bytes(pixels)
  manifest.append(f'{key}\t{ds.Columns}\t{ds.Rows}\t{samples}\t{bits}')
  item['frames'].append({'index':i,'width':int(ds.Columns),'height':int(ds.Rows),'samples':samples,'bits':bits,'component_count':int(raw[i].size),'decoded_bytes':len(pixels),'encoded_bytes':len(data),'encoded_sha256':hashlib.sha256(data).hexdigest(),'pixels_sha256':hashlib.sha256(pixels).hexdigest()})
 results.append(item)
(target/'manifest.tsv').write_text('\n'.join(manifest)+'\n');(target/'result.json').write_text(json.dumps({'pydicom':pydicom.__version__,'numpy':np.__version__,'sources':results},indent=2)+'\n');print('wrote',len(manifest),'packaged frame controls')
