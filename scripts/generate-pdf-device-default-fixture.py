#!/usr/bin/env python3
"""Generate an authored CC0 DefaultCMYK/context-isolation PDF and ICC profiles."""
import hashlib,json,struct
from pathlib import Path
root=Path(__file__).resolve().parents[1]/'tests/fixtures/pdf'
u32=lambda v:struct.pack('>I',v)
def profile(xyz):
 h=bytearray(128);h[4:8]=b'rrrh';h[8:12]=u32(0x02100000);h[12:16]=b'scnr';h[16:20]=b'CMYK';h[20:24]=b'XYZ ';h[24:36]=struct.pack('>6H',2026,10,7,0,0,0);h[36:40]=b'acsp';h[40:44]=b'APPL';h[68:80]=struct.pack('>3i',*[round(v*65536) for v in [.9642,1,.8249]]);h[80:84]=b'rrrh'
 lut=b'mft2'+bytes(4)+bytes([4,3,2,0])+struct.pack('>9i',65536,0,0,0,65536,0,0,0,65536)+struct.pack('>2H',2,2)+struct.pack('>8H',*([0,65535]*4))+struct.pack('>48H',*([max(0,min(65535,round(v*32768))) for v in xyz]*16))+struct.pack('>6H',*([0,65535]*3))
 white=b'XYZ '+bytes(4)+struct.pack('>3i',*[round(v*65536) for v in [.9642,1,.8249]])
 tags=[(b'A2B0',lut),(b'wtpt',white)];offset=128+4+12*len(tags);table=bytearray(u32(len(tags)));data=bytearray()
 for name,value in tags:
  table+=name+u32(offset)+u32(len(value));data+=value;pad=(-len(value))%4;data+=bytes(pad);offset+=len(value)+pad
 output=h+table+data;output[:4]=u32(len(output));return bytes(output)
red=profile([.4360747,.2225045,.0139322]);blue=profile([.1430804,.0606169,.7141733])
objects=[]
def add(value):objects.append(value);return len(objects)
def stream(dictionary,data):return dictionary+b' /Length '+str(len(data)).encode()+b' >>\nstream\n'+data+b'\nendstream'
add(b'<< /Type /Catalog /Pages 2 0 R >>');add(b'<< /Type /Pages /Kids [3 0 R] /Count 1 >>');add(b'PLACEHOLDER');add(stream(b'<<',b'q /R Do Q q 1 0 0 1 72 0 cm /B Do Q'))
pr=add(stream(b'<< /N 4 /Alternate /DeviceCMYK',red));pb=add(stream(b'<< /N 4 /Alternate /DeviceCMYK',blue))
points=[(0,0),(0,1),(0,2),(0,3),(1,3),(2,3),(3,3),(3,2),(3,1),(3,0),(2,0),(1,0),(1,1),(1,2),(2,2),(2,1)]
mesh=bytes([0])+b''.join(struct.pack('>II',x*0xffffffff//3,y*0xffffffff//3) for x,y in points)+bytes([0,255,0,0]*4)
g=add(stream(b'<< /ShadingType 7 /ColorSpace /DeviceCMYK /BitsPerCoordinate 32 /BitsPerComponent 8 /BitsPerFlag 8 /Decode [0 8 0 8 0 1 0 1 0 1 0 1]',mesh))
pat=add(f'<< /Type /Pattern /PatternType 2 /Shading {g} 0 R /Matrix [1 0 0 1 32 0] >>'.encode())
id_=add(stream(b'<< /Type /XObject /Subtype /Image /Width 1 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceCMYK',bytes([0,255,0,0])))
inamed=add(stream(b'<< /Type /XObject /Subtype /Form /BBox [0 0 8 8]',b'q 8 0 0 8 0 0 cm BI /W 1 /H 1 /BPC 8 /CS /Local ID '+bytes([0,255,0,0])+b' EI Q'))
f=add(b'<< /FunctionType 2 /Domain [0 1] /C0 [0 1 0 0] /C1 [0 1 0 0] /N 1 >>')
content=b'0 1 0 0 k 0 0 8 8 re f /DeviceCMYK cs 0 1 0 0 scn 8 0 8 8 re f q 1 0 0 1 16 0 cm /G sh Q q 8 0 0 8 24 0 cm /ID Do Q /Pattern cs /P scn 32 0 8 8 re f q 1 0 0 1 40 0 cm /IN Do Q /Idx cs 0 scn 48 0 8 8 re f /Sep cs 1 scn 56 0 8 8 re f /DN cs 1 scn 64 0 8 8 re f'
forms=[]
for p in [pr,pb]:
 res=f'/Resources << /ColorSpace << /DefaultCMYK [/ICCBased {p} 0 R] /Local [/ICCBased {p} 0 R] /Idx [/Indexed /DeviceCMYK 0 <00ff0000>] /Sep [/Separation /Ink /DeviceCMYK {f} 0 R] /DN [/DeviceN [/Ink] /DeviceCMYK {f} 0 R] >> /Shading << /G {g} 0 R >> /Pattern << /P {pat} 0 R >> /XObject << /ID {id_} 0 R /IN {inamed} 0 R >> >>'
 forms.append(add(stream(('<< /Type /XObject /Subtype /Form /BBox [0 0 72 8] '+res).encode(),content)))
objects[2]=f'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 144 8] /Resources << /XObject << /R {forms[0]} 0 R /B {forms[1]} 0 R >> >> /Contents 4 0 R >>'.encode()
output=bytearray(b'%PDF-1.7\n');offsets=[0]
for i,obj in enumerate(objects,1):offsets.append(len(output));output+=f'{i} 0 obj\n'.encode()+obj+b'\nendobj\n'
xref=len(output);output+=f'xref\n0 {len(offsets)}\n0000000000 65535 f \n'.encode()
for offset in offsets[1:]:output+=f'{offset:010d} 00000 n \n'.encode()
output+=f'trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n'.encode()
path=root/'device-default-cmyk-contexts.pdf';path.write_bytes(output)
path.with_suffix('.json').write_text(json.dumps({'license':'CC0','sha256':hashlib.sha256(output).hexdigest(),'profile_red_sha256':hashlib.sha256(red).hexdigest(),'profile_blue_sha256':hashlib.sha256(blue).hexdigest(),'construction':'Two nested forms use distinct constant-XYZ 4-channel ICC mft2 profiles. Same shading, shading pattern, CMYK image, and named-color inline-image form objects reused in both scopes. Nine cells: k, DeviceCMYK cs/scn, direct mesh, image, pattern mesh, inherited named inline image, Indexed, Separation, DeviceN. Left form should red, right blue. No source input profile or artwork copied.','dimensions':[144,8]},indent=2)+'\n')

# Identical mesh dictionaries and transforms, distinct payload colors; clips
# select opposite halves so a dictionary-only shading cache key is observable.
geometry=mesh[:129]
colors=[bytes([255,0,0]*4),bytes([0,0,255]*4)]
content=b'q 0 0 8 8 re W n /R sh Q q 8 0 8 8 re W n /B sh Q'
extra=[b'<< /Type /Catalog /Pages 2 0 R >>',b'<< /Type /Pages /Kids [3 0 R] /Count 1 >>',b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 16 8] /Resources << /Shading << /R 5 0 R /B 6 0 R >> >> /Contents 4 0 R >>',stream(b'<<',content)]
extra += [stream(b'<< /ShadingType 7 /ColorSpace /DeviceRGB /BitsPerCoordinate 32 /BitsPerComponent 8 /BitsPerFlag 8 /Decode [0 16 0 8 0 1 0 1 0 1]',geometry+c) for c in colors]
output=bytearray(b'%PDF-1.7\n');offsets=[0]
for i,obj in enumerate(extra,1):offsets.append(len(output));output+=f'{i} 0 obj\n'.encode()+obj+b'\nendobj\n'
xref=len(output);output+=f'xref\n0 {len(offsets)}\n0000000000 65535 f \n'.encode()
for offset in offsets[1:]:output+=f'{offset:010d} 00000 n \n'.encode()
output+=f'trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n'.encode()
path=root/'mesh-equal-dictionaries-distinct-streams.pdf';path.write_bytes(output);path.with_suffix('.json').write_text(json.dumps({'license':'CC0','sha256':hashlib.sha256(output).hexdigest(),'construction':'Two type-7 streams with identical dictionary bytes and transforms, red versus blue payloads. Left/right clip rectangles select halves. Must not share encoded shading cache entries.','dimensions':[16,8]},indent=2)+'\n')
