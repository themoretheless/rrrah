#!/usr/bin/env python3
"""Independent encoded-RGB light change on the pinned perspective fixtures."""
import hashlib,json,pathlib
from PIL import Image,__version__
from fractions import Fraction
root=pathlib.Path(__file__).resolve().parents[1]
source=root/'crates/rrrah-dedup/tests/fixtures/photos-projective'
out=root/'crates/rrrah-dedup/tests/fixtures/photos-projective-light'
out.mkdir(exist_ok=True)
rows=[]
for identifier in [2414,2418,2883,5025,1425,5495]:
    path=source/f'{identifier}-perspective.png'
    image=Image.open(path).convert('RGB')
    gain,offset=0.65,0.08
    table=[round(Fraction(65*v+2040,100)) for v in range(256)]
    destination=out/f'{identifier}-perspective-light.png'
    image.point(table*3).save(destination)
    rows.append({'id':identifier,'source':str(path.relative_to(root)),'source_sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'output':str(destination.relative_to(root)),'output_sha256':hashlib.sha256(destination.read_bytes()).hexdigest(),'size':list(image.size)})
(out/'manifest.json').write_text(json.dumps({'pillow_version':__version__,'encoded_rgb_gain':0.65,'encoded_rgb_offset':0.08,'rounding':'Exact rational (65*v+2040)/100, nearest even on 8-bit channels','clipping':False,'fixtures':rows,'scope':'Six authored combined perspective/light development fixtures, not captured illumination pairs or independent semantic labels'},indent=2)+'\n')
