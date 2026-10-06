#!/usr/bin/env python3
"""CC0-authored PDF page-geometry fixtures, independent of renderers."""
from pathlib import Path
root=Path(__file__).resolve().parents[1]/'tests/fixtures/pdf'
for name,extra in [('geometry-0',''),('geometry-90','/Rotate 90'),('geometry-180','/Rotate 180'),('geometry-270','/Rotate 270'),('geometry-crop','/CropBox [2 4 8 16]'),('geometry-unit','/UserUnit 2')]:
    objects=[b'<< /Type /Catalog /Pages 2 0 R >>',b'<< /Type /Pages /Kids [3 0 R] /Count 1 >>',f'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 20] {extra} /Resources << >> /Contents 4 0 R >>'.encode(),None]
    stream=b'1 0 0 rg 0 0 10 10 re f 0 1 0 rg 0 10 10 10 re f\n'
    objects[3]=b'<< /Length '+str(len(stream)).encode()+b' >>\nstream\n'+stream+b'endstream'
    output=bytearray(b'%PDF-1.7\n');offsets=[0]
    for index,obj in enumerate(objects,1):
        offsets.append(len(output));output.extend(f'{index} 0 obj\n'.encode()+obj+b'\nendobj\n')
    xref=len(output);output.extend(f'xref\n0 {len(offsets)}\n0000000000 65535 f \n'.encode())
    for offset in offsets[1:]:output.extend(f'{offset:010} 00000 n \n'.encode())
    output.extend(f'trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n'.encode())
    (root/f'{name}.pdf').write_bytes(output)
