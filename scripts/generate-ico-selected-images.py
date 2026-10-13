#!/usr/bin/env python3
"""CC0 authored ICO directory with Pillow PNG payloads and exact RGBA oracles."""
from pathlib import Path
from PIL import Image
import PIL
import io, struct, json, hashlib
root=Path(__file__).resolve().parents[1]/'tests/fixtures/data'
entries=[(2,2,[255,0,0,128]),(3,1,[0,255,0,64]),(1,3,[0,0,255,255])]
streams=[]
for width,height,pixel in entries:
    output=io.BytesIO();Image.new('RGBA',(width,height),tuple(pixel)).save(output,format='PNG')
    stream=output.getvalue()
    with Image.open(io.BytesIO(stream)) as decoded:
        assert decoded.convert('RGBA').tobytes()==bytes(pixel)*(width*height)
    streams.append(stream)
offset=6+16*len(entries)
output=bytearray(struct.pack('<HHH',0,1,len(entries)))
for (width,height,_),stream in zip(entries,streams):
    output+=struct.pack('<BBBBHHII',width,height,0,0,1,32,len(stream),offset)
    offset+=len(stream)
output+=b''.join(streams)
path=root/'ico-selected-images.ico';path.write_bytes(output)
(root/'ico-selected-images.json').write_text(json.dumps({'license':'CC0-1.0','producer':f'Pillow{PIL.__version__} PNG payloads; authored ICO directory','file':path.name,'sha256':hashlib.sha256(output).hexdigest(),'directory_entries':entries,'quality_order':[0,2,1],'scope':'Highest depth/area first; final-entry tie preserves existing codec default; exact alpha/sample payloads independently decoded'},indent=2)+'\n')
