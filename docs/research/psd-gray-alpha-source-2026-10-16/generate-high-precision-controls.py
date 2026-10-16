#!/usr/bin/env python3
"""Independent psd-tools writer/reader controls, not camera/application originals."""
from pathlib import Path
import json, hashlib, struct
from psd_tools import PSDImage
from psd_tools.constants import Compression
from importlib.metadata import version
folder = Path(__file__).resolve().parent/'high-precision-controls'
folder.mkdir(exist_ok=True)
rows=[]
for depth in [16,32]:
    for mode in ['RGBA','LA']:
        for file_version in [1,2]:
            image=PSDImage.new(mode,(4,1),depth=depth,compression=Compression.RLE)
            image._record.header.version=file_version
            alphas=[0.,.25,.5,1.]
            colors=[.125,.25,.5,.75] if depth==16 else [.125,.5,2.,4.]
            planes=[]
            color_planes=3 if mode=='RGBA' else 1
            for channel in range(color_planes+1):
                values=alphas if channel==color_planes else [c*a+1-a for c,a in zip(colors,alphas)]
                planes.append(b''.join(struct.pack('>H',round(v*65535)) if depth==16 else struct.pack('>f',v) for v in values))
            image._record.image_data.set_data(planes,image._record.header)
            path=folder/f'{mode.lower()}-{depth}-v{file_version}.psd'
            if file_version==2:path=path.with_suffix('.psb')
            image.save(path)
            reopened=PSDImage.open(path)
            values=reopened.numpy()
            assert values.shape==(1,4,color_planes+1)
            oracle=path.with_suffix(path.suffix+'.f32le')
            oracle.write_bytes(values.astype('<f4').tobytes())
            rows.append({'fixture':path.name,'depth':depth,'mode':mode,'version':file_version,
                'source_sha256':hashlib.sha256(path.read_bytes()).hexdigest(),
                'oracle_sha256':hashlib.sha256(oracle.read_bytes()).hexdigest(),
                'expected_native_normalized':values.tolist(),'producer':'psd-tools'+version('psd-tools'),
                'scope':'authored prematted planes through independent writer/reader, not original Photoshop producer'})
(folder/'manifest.json').write_text(json.dumps(rows,indent=2)+'\n')
print(json.dumps({'controls':len(rows),'status':'producer/reference generated; native support not yet claimed'}))
