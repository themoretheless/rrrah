#!/usr/bin/env python3
"""CC0 exact projected-area and ordered-patch controls (PDF Reference1.3 p234)."""
from pathlib import Path
import hashlib,json
root=Path(__file__).resolve().parents[1];directory=root/'tests/fixtures/pdf'
order=[(0,0),(0,1),(0,2),(0,3),(1,3),(2,3),(3,3),(3,2),(3,1),(3,0),(2,0),(1,0),(1,1),(1,2),(2,2),(2,1)]
def patch(xs,color,kind=7):
 return bytes([0])+bytes(z for i,j in (order if kind==7 else order[:12]) for z in [xs[i],85*j])+bytes(color)*4
def pdf(payload,decode,kind=7,background=False):
 content=b'/Pattern cs /P1 scn 0 0 3 3 re f\n' if background else b'/Sh1 sh\n'
 page=b'<</Type/Page/Parent 2 0 R/MediaBox[0 0 3 3]/Contents 4 0 R/Resources<</Pattern<</P1<</Type/Pattern/PatternType 2/Shading 5 0 R>>>>>>>>' if background else b'<</Type/Page/Parent 2 0 R/MediaBox[0 0 3 3]/Contents 4 0 R/Resources<</Shading<</Sh1 5 0 R>>>>>>'
 objects=[b'<</Type/Catalog/Pages 2 0 R>>',b'<</Type/Pages/Kids[3 0 R]/Count 1>>',page,b'<</Length '+str(len(content)).encode()+b'>>\nstream\n'+content+b'endstream',b'<</ShadingType '+str(kind).encode()+b'/ColorSpace/DeviceRGB'+(b'/Background[0 1 0]' if background else b'')+b'/BitsPerCoordinate 8/BitsPerComponent 8/BitsPerFlag 8/Decode['+decode+b']/Length '+str(len(payload)).encode()+b'>>\nstream\n'+payload+b'\nendstream']
 out=bytearray(b'%PDF-1.7\n');offsets=[0]
 for i,obj in enumerate(objects,1):offsets.append(len(out));out.extend(str(i).encode()+b' 0 obj\n'+obj+b'\nendobj\n')
 xref=len(out);out.extend(b'xref\n0 6\n0000000000 65535 f \n')
 for pos in offsets[1:]:out.extend(f'{pos:010d} 00000 n \n'.encode())
 out.extend(b'trailer\n<</Size 6/Root 1 0 R>>\nstartxref\n'+str(xref).encode()+b'\n%%EOF\n');return bytes(out)
rows=[]
for name,payload,decode,expected,math in [
 ('tensor-folded-solid.pdf',patch([0,255,255,0],[0,0,0]),b'0 2 0 3 0 1 0 1 0 1',[0,0,0,255,0,0,0,128,0,0,0,0]*3,'x(u)=6u(1-u), y(v)=3v; image=[0,1.5]x[0,3]; solid black union; half pixel rounds127.5 to128'),
 ('tensor-partial-overlap.pdf',patch([0,51,102,153],[255,0,0])+patch([51,102,153,204],[0,0,255]),b'0 2.5 0 3 0 1 0 1 0 1',[128,0,128,255,0,0,255,255,0,0,0,0]*3,'red rectangle[0,1.5] then blue[.5,2], y=[0,3]; later patch owns overlap; first pixel half red/blue, second entirely blue'),
]:
 data=pdf(payload,decode);(directory/name).write_bytes(data)
 rows.append(dict(file=name,sha256=hashlib.sha256(data).hexdigest(),expected_rgba8=expected,math=math))
(directory/'tensor-overlap-controls.json').write_text(json.dumps(dict(license='CC0 authored control geometry',spec='https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/pdfreference1.3.pdf#page=249',scope='Exact3x3 projected coverage and source-order ownership; expected values derived from geometry, not renderer output',rows=rows),indent=2)+'\n')

coons_rows=[]
for name,xs,colors,decode,expected,math in [
 ('coons-folded-solid.pdf',[[0,255,255,0]],[[0,0,0]],b'0 2 0 3 0 1 0 1 0 1',rows[0]['expected_rgba8'],rows[0]['math']),
 ('coons-partial-overlap.pdf',[[0,51,102,153],[51,102,153,204]],[[255,0,0],[0,0,255]],b'0 2.5 0 3 0 1 0 1 0 1',rows[1]['expected_rgba8'],rows[1]['math']),
]:
 data=pdf(b''.join(patch(x,c,6) for x,c in zip(xs,colors)),decode,6)
 (directory/name).write_bytes(data)
 coons_rows.append(dict(file=name,sha256=hashlib.sha256(data).hexdigest(),expected_rgba8=expected,math=math))
(directory/'coons-overlap-controls.json').write_text(json.dumps(dict(license='CC0 authored control geometry',scope='Type 6 Coons boundaries reproduce the same separable exact projected controls as Type 7',rows=coons_rows),indent=2)+'\n')

background=pdf(patch([0,255,255,0],[0,0,0]),b'0 2 0 3 0 1 0 1 0 1',background=True)
(directory/'tensor-folded-background.pdf').write_bytes(background)
(directory/'tensor-folded-background.json').write_text(json.dumps(dict(license='CC0 authored control geometry',sha256=hashlib.sha256(background).hexdigest(),expected_rgba8=[0,0,0,255,0,128,0,255,0,255,0,255]*3,math='Shading pattern fills 3x3 with green background; folded black projected union covers [0,1.5]x[0,3]; green fills uncovered subpixel area, not direct sh background'),indent=2)+'\n')
