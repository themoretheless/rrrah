#!/usr/bin/env python3
"""Authored CC0 planar tensor patch fixtures with explicit point/color ordering."""
from pathlib import Path
import hashlib,json,struct
root=Path(__file__).resolve().parents[1]/'tests/fixtures/pdf'
# PDF type-7 boundary order followed by four interior points.
points=[(0,0),(0,1),(0,2),(0,3),(1,3),(2,3),(3,3),(3,2),(3,1),(3,0),(2,0),(1,0),(1,1),(1,2),(2,2),(2,1)]
for name,space,colors in [('tensor-fractional-background-rgb','DeviceRGB',[[255,0,0]]*4),('tensor-fractional-rgb','DeviceRGB',[[255,0,0]]*4),('tensor-solid-rgb','DeviceRGB',[[255,0,0]]*4),('tensor-corners-rgb','DeviceRGB',[[255,0,0],[0,255,0],[0,0,255],[255,255,255]]),('tensor-solid-cmyk','DeviceCMYK',[[0,255,0,0]]*4)]:
 data=bytes([0])+b''.join(struct.pack('>II',x*0xffffffff//3,y*0xffffffff//3) for x,y in points)+bytes(v for color in colors for v in color)
 content=b'/G sh\n';components=len(colors[0]);decode=('0.25 31.25 0.25 31.25 ' if name.startswith('tensor-fractional') else '0 32 0 32 ')+' '.join(['0 1']*components)
 objects=[b'<< /Type /Catalog /Pages 2 0 R >>',b'<< /Type /Pages /Kids [3 0 R] /Count 1 >>',b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 32 32] /Resources << /Shading << /G 5 0 R >> >> /Contents 4 0 R >>',b'<< /Length '+str(len(content)).encode()+b' >>\nstream\n'+content+b'endstream',f'<< /ShadingType 7 {'/Background [0 0 1]' if name=='tensor-fractional-background-rgb' else ''} /ColorSpace /{space} /BitsPerCoordinate 32 /BitsPerComponent 8 /BitsPerFlag 8 /Decode [{decode}] /Length {len(data)} >>\nstream\n'.encode()+data+b'\nendstream']
 output=bytearray(b'%PDF-1.7\n');offsets=[0]
 for i,obj in enumerate(objects,1):offsets.append(len(output));output.extend(f'{i} 0 obj\n'.encode()+obj+b'\nendobj\n')
 xref=len(output);output.extend(f'xref\n0 {len(offsets)}\n0000000000 65535 f \n'.encode())
 for offset in offsets[1:]:output.extend(f'{offset:010} 00000 n \n'.encode())
 output.extend(f'trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n'.encode())
 path=root/(name+'.pdf');path.write_bytes(output);path.with_suffix('.json').write_text(json.dumps({'license':'CC0','sha256':hashlib.sha256(output).hexdigest(),'construction':'32x32 single planar type-7 tensor patch; no mask or blend','boundary_and_interior_points':points,'corner_components_u8':colors,'color_space':space},indent=2)+'\n')

# Shared right edge (flag 2), independently padded records for 2/8-bit flags.
def packed_patch(flag, point_list, colors, flag_bits):
 bits=f'{flag:0{flag_bits}b}'
 for x,y in point_list:
  bits+=f'{round(x*0xffffffff/6):032b}{round(y*0xffffffff/3):032b}'
 for color in colors:
  for value in color:bits+=f'{value:08b}'
 bits+='0'*((-len(bits))%8)
 return bytes(int(bits[i:i+8],2) for i in range(0,len(bits),8))
for flag_bits in [8,4,2]:
 first=packed_patch(0,points,[[255,0,0]]*4,flag_bits)
 second_points=[(3+x,3-y) for x,y in points]
 second=packed_patch(2,second_points[4:],[[0,0,255]]*2,flag_bits)
 data=first+second;content=b'/G sh\n'
 objects=[b'<< /Type /Catalog /Pages 2 0 R >>',b'<< /Type /Pages /Kids [3 0 R] /Count 1 >>',b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 64 32] /Resources << /Shading << /G 5 0 R >> >> /Contents 4 0 R >>',b'<< /Length '+str(len(content)).encode()+b' >>\nstream\n'+content+b'endstream',f'<< /ShadingType 7 /ColorSpace /DeviceRGB /BitsPerCoordinate 32 /BitsPerComponent 8 /BitsPerFlag {flag_bits} /Decode [0 64 0 32 0 1 0 1 0 1] /Length {len(data)} >>\nstream\n'.encode()+data+b'\nendstream']
 output=bytearray(b'%PDF-1.7\n');offsets=[0]
 for i,obj in enumerate(objects,1):offsets.append(len(output));output.extend(f'{i} 0 obj\n'.encode()+obj+b'\nendobj\n')
 xref=len(output);output.extend(f'xref\n0 {len(offsets)}\n0000000000 65535 f \n'.encode())
 for offset in offsets[1:]:output.extend(f'{offset:010} 00000 n \n'.encode())
 output.extend(f'trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n'.encode())
 path=root/f'tensor-shared-edge-{flag_bits}bit.pdf';path.write_bytes(output);path.with_suffix('.json').write_text(json.dumps({'license':'CC0','sha256':hashlib.sha256(output).hexdigest(),'construction':'64x32 two planar patches, flag2 inherited right edge, independently byte padded records','flag_bits':flag_bits,'expected':'left half solid red; right half red-to-blue horizontal linear gradient'},indent=2)+'\n')
