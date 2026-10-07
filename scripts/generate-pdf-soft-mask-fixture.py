#!/usr/bin/env python3
"""Generate a CC0 soft-mask identity regression with one shared group."""
import hashlib
import json
from pathlib import Path

def stream(dictionary, data):
    return dictionary + b' /Length ' + str(len(data)).encode() + b' >>\nstream\n' + data + b'\nendstream'

objects = [
    b'<< /Type /Catalog /Pages 2 0 R >>',
    b'<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
    b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 24 8] /Resources << /ExtGState << /L 5 0 R /A 6 0 R /T 8 0 R >> >> /Contents 4 0 R >>',
    stream(b'<<', b'q /L gs 0 g 0 0 8 8 re f Q q /A gs 0 g 8 0 8 8 re f Q q /T gs 0 g 16 0 8 8 re f Q'),
    b'<< /SMask << /S /Luminosity /G 7 0 R >> >>',
    b'<< /SMask << /S /Alpha /G 7 0 R >> >>',
    stream(b'<< /Type /XObject /Subtype /Form /BBox [0 0 24 8] /Group << /S /Transparency /CS /DeviceRGB /I true >> /Resources << >>', b'0 0 1 rg 0 0 24 8 re f'),
    b'<< /SMask << /S /Luminosity /G 7 0 R /TR 9 0 R >> >>',
    b'<< /FunctionType 2 /Domain [0 1] /C0 [1] /C1 [1] /N 1 >>',
]
output = bytearray(b'%PDF-1.7\n')
offsets = [0]
for number, obj in enumerate(objects, 1):
    offsets.append(len(output))
    output += f'{number} 0 obj\n'.encode() + obj + b'\nendobj\n'
xref = len(output)
output += f'xref\n0 {len(offsets)}\n0000000000 65535 f \n'.encode()
for offset in offsets[1:]:
    output += f'{offset:010d} 00000 n \n'.encode()
output += f'trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n'.encode()
path = Path(__file__).resolve().parents[1] / 'tests/fixtures/pdf/soft-mask-shared-group.pdf'
path.write_bytes(output)
path.with_suffix('.json').write_text(json.dumps({'license': 'CC0', 'sha256': hashlib.sha256(output).hexdigest(), 'dimensions': [24, 8], 'construction': 'Three masks share a solid-blue group: luminosity, alpha, and luminosity with constant-one transfer. All three painted black on transparent canvas; expected alpha approximately 28, 255, 255.'}, indent=2) + '\n')

# Independent primary-color luminosity bands.
objects[3] = stream(b'<<', b'/L gs 0 g 0 0 24 8 re f')
objects[6] = stream(b'<< /Type /XObject /Subtype /Form /BBox [0 0 24 8] /Group << /S /Transparency /CS /DeviceRGB /I true >> /Resources << >>', b'1 0 0 rg 0 0 8 8 re f 0 1 0 rg 8 0 8 8 re f 0 0 1 rg 16 0 8 8 re f')
output = bytearray(b'%PDF-1.7\n')
offsets = [0]
for number, obj in enumerate(objects, 1):
    offsets.append(len(output))
    output += f'{number} 0 obj\n'.encode() + obj + b'\nendobj\n'
xref = len(output)
output += f'xref\n0 {len(offsets)}\n0000000000 65535 f \n'.encode()
for offset in offsets[1:]:
    output += f'{offset:010d} 00000 n \n'.encode()
output += f'trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n'.encode()
path = path.with_name('soft-mask-device-rgb-bands.pdf')
path.write_bytes(output)
path.with_suffix('.json').write_text(json.dumps({'license':'CC0','sha256':hashlib.sha256(output).hexdigest(),'dimensions':[24,8],'construction':'DeviceRGB luminosity mask with red, green, blue bands; black foreground.'},indent=2)+'\n')

# One shared mask group inherits distinct ICC profiles from two form scopes.
import runpy
profile = runpy.run_path(str(Path(__file__).with_name('generate-pdf-device-default-fixture.py')))['profile']
objects = [
    b'<< /Type /Catalog /Pages 2 0 R >>',
    b'<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
    b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 16 8] /Resources << /XObject << /R 5 0 R /B 6 0 R >> >> /Contents 4 0 R >>',
    stream(b'<<', b'q 0 0 8 8 re W n /R Do Q q 8 0 8 8 re W n /B Do Q'),
]
for number in [9,10]:
    resources = b'/Resources << /ColorSpace << /DefaultRGB [/ICCBased '+str(number).encode()+b' 0 R] >> /ExtGState << /M 7 0 R >> >>'
    objects.append(stream(b'<< /Type /XObject /Subtype /Form /BBox [0 0 16 8] '+resources, b'/M gs 0 g 0 0 16 8 re f'))
objects += [
    b'<< /SMask << /S /Luminosity /BC [0 1 0] /G 8 0 R >> >>',
    stream(b'<< /Type /XObject /Subtype /Form /BBox [0 0 16 8] /Group << /S /Transparency /CS /DeviceRGB /I true >>', b''),
    stream(b'<< /N 3 /Alternate /DeviceRGB',profile([.4360747,.2225045,.0139322],3)),
    stream(b'<< /N 3 /Alternate /DeviceRGB',profile([.1430804,.0606169,.7141733],3)),
]
output = bytearray(b'%PDF-1.7\n')
offsets = [0]
for number, obj in enumerate(objects, 1):
    offsets.append(len(output))
    output += f'{number} 0 obj\n'.encode() + obj + b'\nendobj\n'
xref = len(output)
output += f'xref\n0 {len(offsets)}\n0000000000 65535 f \n'.encode()
for offset in offsets[1:]:
    output += f'{offset:010d} 00000 n \n'.encode()
output += f'trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n'.encode()
path = path.with_name('soft-mask-inherited-profile.pdf')
path.write_bytes(output)
path.with_suffix('.json').write_text(json.dumps({'license':'CC0','sha256':hashlib.sha256(output).hexdigest(),'dimensions':[16,8],'construction':'Same empty mask group and identical CTM, distinct inherited DefaultRGB ICC profile scopes; constant green BC becomes red versus blue. Cache identity must include inherited resources.'},indent=2)+'\n')

# Alpha masks do not require a group blending color-space entry.
objects = [
    b'<< /Type /Catalog /Pages 2 0 R >>',
    b'<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
    b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 16 8] /Resources << /ExtGState << /A 5 0 R >> >> /Contents 4 0 R >>',
    stream(b'<<', b'/A gs 0 g 0 0 16 8 re f'),
    b'<< /SMask << /S /Alpha /G 6 0 R >> >>',
    stream(b'<< /Type /XObject /Subtype /Form /BBox [0 0 16 8] /Group << /S /Transparency /I true >> /Resources << >>',b'0 0 1 rg 0 0 8 8 re f'),
]
output = bytearray(b'%PDF-1.7\n')
offsets = [0]
for number, obj in enumerate(objects, 1):
    offsets.append(len(output))
    output += f'{number} 0 obj\n'.encode() + obj + b'\nendobj\n'
xref = len(output)
output += f'xref\n0 {len(offsets)}\n0000000000 65535 f \n'.encode()
for offset in offsets[1:]:
    output += f'{offset:010d} 00000 n \n'.encode()
output += f'trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n'.encode()
path = path.with_name('soft-mask-alpha-without-group-cs.pdf')
path.write_bytes(output)
path.with_suffix('.json').write_text(json.dumps({'license':'CC0','sha256':hashlib.sha256(output).hexdigest(),'dimensions':[16,8],'construction':'Alpha mask group omits optional CS; left half opaque artwork, right half empty. Foreground black; expected alpha 255 left, 0 right.'},indent=2)+'\n')
