#!/usr/bin/env python3
"""Authored CC0 CUR entries exercise tied largest images with distinct hotspots."""
from pathlib import Path
from PIL import Image
import PIL
import io,struct,json,hashlib
root=Path(__file__).resolve().parents[1]/'tests/fixtures/data'
entries=[(2,2,[255,0,0,128],[0,1]),(2,2,[0,255,0,64],[1,0]),(1,1,[0,0,255,255],[0,0])]
streams=[]
for width,height,pixel,_ in entries:
    output=io.BytesIO();Image.new('RGBA',(width,height),tuple(pixel)).save(output,format='PNG')
    stream=output.getvalue()
    with Image.open(io.BytesIO(stream)) as decoded:assert decoded.convert('RGBA').tobytes()==bytes(pixel)*(width*height)
    streams.append(stream)
offset=6+16*len(entries);output=bytearray(struct.pack('<HHH',0,2,len(entries)))
for (width,height,_,hotspot),stream in zip(entries,streams):
    output+=struct.pack('<BBBBHHII',width,height,0,0,*hotspot,len(stream),offset);offset+=len(stream)
output+=b''.join(streams)
path=root/'cur-selected-images.cur';path.write_bytes(output)
(root/'cur-selected-images.json').write_text(json.dumps({'license':'CC0-1.0','producer':f'Pillow{PIL.__version__} PNG payloads; authored CUR directory','file':path.name,'sha256':hashlib.sha256(output).hexdigest(),'entries':entries,'quality_order':[0,1,2],'scope':'Default image is first tied largest because final entry is smaller; hotspot must belong to same entry'},indent=2)+'\n')
