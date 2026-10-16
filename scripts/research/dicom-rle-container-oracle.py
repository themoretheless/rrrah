"""Full packaged RLE RGB container frames, source read in place, pydicom oracle."""
from pathlib import Path
import hashlib,json,sys
import numpy as np
import pydicom
source,target=map(Path,sys.argv[1:3]);target.mkdir(parents=True,exist_ok=True);manifest=[];results=[]
for name in ['SC_rgb_rle.dcm','SC_rgb_rle_2frame.dcm','SC_rgb_rle_16bit.dcm','SC_rgb_rle_16bit_2frame.dcm']:
 path=source/name;d=pydicom.dcmread(path);d.pixel_array_options(decoding_plugin='pydicom');frames=int(d.get('NumberOfFrames',1));bits=int(d.BitsAllocated);a=d.pixel_array.reshape(frames,d.Rows,d.Columns,3)
 for i in range(frames):
  alpha=np.full((d.Rows,d.Columns,1),(1<<bits)-1,dtype=a.dtype);data=np.concatenate([a[i],alpha],axis=-1).astype('<u2' if bits==16 else np.uint8).tobytes();key=f'{name}.{i}.rgba{bits}';(target/key).write_bytes(data)
  scratch=int(d.Rows)*int(d.Columns)*3*(bits//8);manifest.append(f'{name}\t{d.Columns}\t{d.Rows}\t{bits}\t{i}\t{scratch}\t{key}\t{frames}')
  results.append({'name':name,'index':i,'frames':frames,'source_sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'pixels':int(d.Rows*d.Columns),'rgba_bytes':len(data),'scratch':scratch,'peak':path.stat().st_size+len(data)+scratch,'oracle_sha256':hashlib.sha256(data).hexdigest()})
(target/'manifest.tsv').write_text('\n'.join(manifest)+'\n');(target/'result.json').write_text(json.dumps(results,indent=2)+'\n');print('wrote',len(results),'full RLE color frames')
