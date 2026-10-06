#!/usr/bin/env python3
"""CC0 cubic-circle fixture for independent rasterization diagnostics."""
import hashlib
import json
from pathlib import Path
root=Path(__file__).resolve().parents[1]/'tests/fixtures/pdf'
content=b'0 0 0 rg 14 8 m 14 11.3137085 11.3137085 14 8 14 c 4.6862915 14 2 11.3137085 2 8 c 2 4.6862915 4.6862915 2 8 2 c 11.3137085 2 14 4.6862915 14 8 c h f\n'
objects=[b'<< /Type /Catalog /Pages 2 0 R >>',b'<< /Type /Pages /Kids [3 0 R] /Count 1 >>',b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 16 16] /Resources << >> /Contents 4 0 R >>',b'<< /Length '+str(len(content)).encode()+b' >>\nstream\n'+content+b'endstream']
output=bytearray(b'%PDF-1.7\n');offsets=[0]
for index,obj in enumerate(objects,1):
    offsets.append(len(output));output.extend(f'{index} 0 obj\n'.encode()+obj+b'\nendobj\n')
xref=len(output);output.extend(f'xref\n0 {len(offsets)}\n0000000000 65535 f \n'.encode())
for offset in offsets[1:]:output.extend(f'{offset:010} 00000 n \n'.encode())
output.extend(f'trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n'.encode())
p=root/'curve-circle.pdf';p.write_bytes(output)
p.with_suffix('.json').write_text(json.dumps({'license':'CC0','sha256':hashlib.sha256(output).hexdigest(),'construction':'16x16 page, four cubic black circle segments, no mask/color profile/font/image'},indent=2)+'\n')
