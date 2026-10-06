#!/usr/bin/env python3
"""CC0 minimal PDF soft-mask isolation, not renderer-derived fixtures."""
import hashlib
import json
from pathlib import Path
root = Path(__file__).resolve().parents[1] / 'tests/fixtures/pdf'
def stream(data, attributes=b''):
    return b'<< '+attributes+b' /Length '+str(len(data)).encode()+b' >>\nstream\n'+data+b'\nendstream'
for name, masked in [('soft-mask-control',False),('soft-mask-luminosity',True)]:
    content=(b'/GS gs ' if masked else b'')+b'0 0 1 rg 0 0 12 8 re f'
    objects=[b'<< /Type /Catalog /Pages 2 0 R >>',
        b'<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
        b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 12 8] /Resources << /ExtGState << /GS 5 0 R >> >> /Contents 4 0 R >>',
        stream(content),
        b'<< /Type /ExtGState /SMask << /S /Luminosity /G 6 0 R >> >>',
        stream(b'0 g 0 0 4 8 re f 0.5 g 4 0 4 8 re f 1 g 8 0 4 8 re f',
            b'/Type /XObject /Subtype /Form /BBox [0 0 12 8] /Resources << >> /Group << /S /Transparency /CS /DeviceGray >>')]
    output=bytearray(b'%PDF-1.7\n');offsets=[0]
    for index,obj in enumerate(objects,1):
        offsets.append(len(output));output.extend(f'{index} 0 obj\n'.encode()+obj+b'\nendobj\n')
    xref=len(output);output.extend(f'xref\n0 {len(offsets)}\n0000000000 65535 f \n'.encode())
    for offset in offsets[1:]:output.extend(f'{offset:010} 00000 n \n'.encode())
    output.extend(f'trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n'.encode())
    path=root/(name+'.pdf');path.write_bytes(output)
    path.with_suffix('.json').write_text(json.dumps({'license':'CC0','sha256':hashlib.sha256(output).hexdigest(),'construction':'Blue rectangle; optional DeviceGray luminosity soft mask with 0, 0.5, 1 vertical bands'},indent=2)+'\n')
