#!/usr/bin/env python3
"""CC0 TIFF page-profile fixture. Requires Pillow with ImageCms (LittleCMS)."""
from pathlib import Path
from PIL import Image, ImageCms
import PIL
import io
import struct
import zlib
import json
import hashlib

root = Path(__file__).resolve().parents[1] / 'tests/fixtures/data'
srgb = bytearray(ImageCms.ImageCmsProfile(ImageCms.createProfile('sRGB')).tobytes())
struct.pack_into('>6H', srgb, 24, 2026, 1, 1, 0, 0, 0)
srgb[84:100] = bytes(16)
srgb = bytes(srgb)
linear = bytearray(srgb)
count = struct.unpack_from('>I', linear, 128)[0]
changed = set()
for i in range(count):
    tag, offset, size = struct.unpack_from('>4sII', linear, 132 + i * 12)
    if tag in [b'rTRC', b'gTRC', b'bTRC']:
        assert linear[offset:offset+4] == b'para' and size >= 16
        linear[offset+8:offset+size] = b'\0' * (size-8)
        struct.pack_into('>i', linear, offset+12, 65536)  # parametric function0, gamma1
        changed.add(tag)
assert len(changed) == 3
linear = bytes(linear)
# Independent profile parser accepts both distinct profiles.
for profile in [srgb, linear]:
    ImageCms.ImageCmsProfile(io.BytesIO(profile))
profiles = [srgb, linear, srgb]
pixels = bytes([128,64,192,20,220,100])
streams = [zlib.compress(pixels) for _ in profiles]
stride = 2 + 11 * 12 + 4
extra = 8 + len(profiles) * stride
bits_at = []
profile_at = []
stream_at = []
for profile, stream in zip(profiles, streams):
    bits_at.append(extra); profile_at.append(extra+6); stream_at.append(extra+6+len(profile))
    extra += 6 + len(profile) + len(stream)
    extra += extra % 2
output = bytearray(b'II' + struct.pack('<HI',42,8))
for page, (profile, stream) in enumerate(zip(profiles, streams)):
    tags = [(256,4,1,2),(257,4,1,1),(258,3,3,bits_at[page]),(259,3,1,8),
            (262,3,1,2),(273,4,1,stream_at[page]),(274,3,1,1),(277,3,1,3),
            (278,4,1,1),(279,4,1,len(stream)),(34675,7,len(profile),profile_at[page])]
    output += struct.pack('<H',len(tags))
    for tag, kind, count, value in tags:
        raw = struct.pack('<H',value)+b'\0\0' if kind==3 and count==1 else struct.pack('<I',value)
        output += struct.pack('<HHI',tag,kind,count)+raw
    output += struct.pack('<I',8+(page+1)*stride if page<2 else 0)
for profile, stream in zip(profiles,streams):
    output += struct.pack('<3H',8,8,8)+profile+stream
    if len(output)%2: output += b'\0'
path = root / 'tiff-distinct-page-profiles.tif'
path.write_bytes(output)
oracle = bytearray()
for profile in profiles:
    transform = ImageCms.buildTransformFromOpenProfiles(
        ImageCms.ImageCmsProfile(io.BytesIO(profile)),
        ImageCms.ImageCmsProfile(io.BytesIO(srgb)), 'RGB', 'RGB', renderingIntent=1)
    oracle += ImageCms.applyTransform(Image.frombytes('RGB', (2, 1), pixels), transform).tobytes()
assert oracle[:6] == pixels and oracle[12:] == pixels and oracle[6:12] != pixels
path.with_suffix('.rgb8').write_bytes(oracle)
(root/'tiff-distinct-page-profiles.json').write_text(json.dumps({
    'license':'CC0-1.0','producer':{'Pillow':PIL.__version__,'LittleCMS':ImageCms.core.littlecms_version},
    'file':path.name,'sha256':hashlib.sha256(output).hexdigest(),
    'pages':['sRGB transfer','linear gamma1 with sRGB primaries','sRGB transfer'],
    'profiles_sha256':[hashlib.sha256(p).hexdigest() for p in profiles],
    'pixels_rgb8':list(pixels),'dimensions':[2,1],
    'relative_colorimetric_srgb_rgb8':list(oracle),'oracle_sha256':hashlib.sha256(oracle).hexdigest(),
    'scope':'Authored Deflate TIFF structure; LittleCMS-created sRGB profile and explicitly edited gamma1 TRCs, parsed independently',
},indent=2)+'\n')
