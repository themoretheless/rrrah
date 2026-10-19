#!/usr/bin/env python3
"""CC0 curved tensor shading and independent analytic pixel coverage controls."""
from pathlib import Path
import math,struct,json,hashlib
root=Path(__file__).resolve().parents[1]/'tests/fixtures/pdf';order=[(0,0),(0,1),(0,2),(0,3),(1,3),(2,3),(3,3),(3,2),(3,1),(3,0),(2,0),(1,0),(1,1),(1,2),(2,2),(2,1)]
points=[(32*i/3,2+8*j/3+(128/3 if i in (1,2) else 0)) for i,j in order]
data=bytes([0])+b''.join(struct.pack('>II',round(x/32*0xffffffff),round(y/64*0xffffffff)) for x,y in points)+bytes([255,0,0]*4)
content=b'/G sh\n'
objects=[b'<< /Type /Catalog /Pages 2 0 R >>',b'<< /Type /Pages /Kids [3 0 R] /Count 1 >>',b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 32 64] /Resources << /Shading << /G 5 0 R >> >> /Contents 4 0 R >>',b'<< /Length 6 >>\nstream\n'+content+b'endstream',b'<< /ShadingType 7 /ColorSpace /DeviceRGB /BitsPerCoordinate 32 /BitsPerComponent 8 /BitsPerFlag 8 /Decode [0 32 0 64 0 1 0 1 0 1] /Length '+str(len(data)).encode()+b' >>\nstream\n'+data+b'\nendstream']
out=bytearray(b'%PDF-1.7\n');offsets=[0]
for i,obj in enumerate(objects,1):offsets.append(len(out));out.extend(f'{i} 0 obj\n'.encode()+obj+b'\nendobj\n')
xref=len(out);out.extend(f'xref\n0 {len(offsets)}\n0000000000 65535 f \n'.encode())
for offset in offsets[1:]:out.extend(f'{offset:010} 00000 n \n'.encode())
out.extend(f'trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n'.encode())
(root/'tensor-parabolic-rgb.pdf').write_bytes(out)
def integral(k,x):
 a,b=x,x+1;cuts=[a,b]
 for t in [k,k-1]:
  if t<=34:
   r=math.sqrt(8*(34-t))
   cuts.extend(p for p in [16-r,16+r] if a<p<b)
 cuts.sort();total=0
 def primitive(x):return (k-2)*x-2*x*x+x*x*x/24
 for lo,hi in zip(cuts,cuts[1:]):
  m=(lo+hi)/2;v=k-(2+4*m-m*m/8)
  total+=(hi-lo if v>=1 else primitive(hi)-primitive(lo) if v>0 else 0)
 return total
alpha=bytes(round(max(0,min(1,integral(64-row,x)-integral(56-row,x)))*255) for row in range(64) for x in range(32))
(root/'tensor-parabolic-rgb.alpha').write_bytes(alpha)
(root/'tensor-parabolic-rgb.json').write_text(json.dumps({'license':'CC0','construction':'x=32u, y=2+8v+128u(1-u); solid red; page32x64','oracle':'Exact piecewise antiderivative of clamped parabola vertical coverage; independent of triangle code','coordinate_quantization_max':64/(2**32-1)/2,'sha256':hashlib.sha256(out).hexdigest()},indent=2)+'\n')

# The same boundary is an exact Coons patch: its transfinite blend
# reproduces x=32u, y=2+8v+128u(1-u). Interior Tensor points are omitted.
coons_data=data[:1+12*8]+data[1+16*8:]
coons_objects=objects.copy()
coons_objects[-1]=objects[-1].replace(b'/ShadingType 7',b'/ShadingType 6').replace(
    b'/Length '+str(len(data)).encode(),b'/Length '+str(len(coons_data)).encode()).replace(data,coons_data)
out=bytearray(b'%PDF-1.7\n');offsets=[0]
for i,obj in enumerate(coons_objects,1):offsets.append(len(out));out.extend(f'{i} 0 obj\n'.encode()+obj+b'\nendobj\n')
xref=len(out);out.extend(f'xref\n0 {len(offsets)}\n0000000000 65535 f \n'.encode())
for offset in offsets[1:]:out.extend(f'{offset:010} 00000 n \n'.encode())
out.extend(f'trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n'.encode())
(root/'coons-parabolic-rgb.pdf').write_bytes(out)

# A finite, intersecting patch whose curvature cannot meet the depth bound.
# PDF numbers use decimal notation; exponent syntax is not a PDF number.
objects[-1]=objects[-1].replace(b'/Decode [0 32 0 64',b'/Decode [0 32000000000000000000 -2000000000000000000 62000000000000000000')
out=bytearray(b'%PDF-1.7\n');offsets=[0]
for i,obj in enumerate(objects,1):offsets.append(len(out));out.extend(f'{i} 0 obj\n'.encode()+obj+b'\nendobj\n')
xref=len(out);out.extend(f'xref\n0 {len(offsets)}\n0000000000 65535 f \n'.encode())
for offset in offsets[1:]:out.extend(f'{offset:010} 00000 n \n'.encode())
out.extend(f'trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n'.encode())
(root/'tensor-depth-limit.pdf').write_bytes(out)

coons_objects[-1]=coons_objects[-1].replace(b'/Decode [0 32 0 64',b'/Decode [0 32000000000000000000 -2000000000000000000 62000000000000000000')
out=bytearray(b'%PDF-1.7\n');offsets=[0]
for i,obj in enumerate(coons_objects,1):offsets.append(len(out));out.extend(f'{i} 0 obj\n'.encode()+obj+b'\nendobj\n')
xref=len(out);out.extend(f'xref\n0 {len(offsets)}\n0000000000 65535 f \n'.encode())
for offset in offsets[1:]:out.extend(f'{offset:010} 00000 n \n'.encode())
out.extend(f'trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n'.encode())
(root/'coons-depth-limit.pdf').write_bytes(out)
