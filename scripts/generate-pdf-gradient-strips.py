#!/usr/bin/env python3
"""CC0 constant-color center-sample strips to isolate CMYK gradient interpolation."""
import hashlib,json
from pathlib import Path
root=Path(__file__).resolve().parents[1]/'tests/fixtures/pdf'
for name,space,start,end in [('gradient-center-strips-rgb','DeviceRGB','1 0 0','0 0 1'),('gradient-center-strips-cmyk','DeviceCMYK','1 0 0 0','0 1 0 0')]:
    content=b''
    for x in range(12):
        t=(x+.5)/12
        color=f'{1-t:.12f} 0 {t:.12f} rg' if space=='DeviceRGB' else f'{1-t:.12f} {t:.12f} 0 0 k'
        content+=f'{color} {x} 0 1 8 re f\n'.encode()
    objects=[b'<< /Type /Catalog /Pages 2 0 R >>',b'<< /Type /Pages /Kids [3 0 R] /Count 1 >>',b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 12 8] /Resources << /Shading << /G 5 0 R >> >> /Contents 4 0 R >>',b'<< /Length '+str(len(content)).encode()+b' >>\nstream\n'+content+b'endstream',f'<< /ShadingType 2 /ColorSpace /{space} /Coords [0 0 12 0] /Function 6 0 R /Extend [true true] >>'.encode(),f'<< /FunctionType 2 /Domain [0 1] /C0 [{start}] /C1 [{end}] /N 1 >>'.encode()]
    output=bytearray(b'%PDF-1.7\n');offsets=[0]
    for index,obj in enumerate(objects,1):
        offsets.append(len(output));output.extend(f'{index} 0 obj\n'.encode()+obj+b'\nendobj\n')
    xref=len(output);output.extend(f'xref\n0 {len(offsets)}\n0000000000 65535 f \n'.encode())
    for offset in offsets[1:]:output.extend(f'{offset:010} 00000 n \n'.encode())
    output.extend(f'trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n'.encode())
    path=root/(name+'.pdf');path.write_bytes(output);path.with_suffix('.json').write_text(json.dumps({'license':'CC0','sha256':hashlib.sha256(output).hexdigest(),'construction':f'12x8 constant-color strips in {space}, centered ramp samples, no mask/blend'},indent=2)+'\n')
