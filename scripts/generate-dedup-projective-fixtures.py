"""Create independently Pillow-warped perspective derivatives of pinned previews."""
from pathlib import Path
import hashlib,json
from PIL import Image
root=Path(__file__).resolve().parent.parent
source=root/'crates/rrrah-dedup/tests/fixtures/photos-heldout'
output=root/'crates/rrrah-dedup/tests/fixtures/photos-projective';output.mkdir(exist_ok=True)
coefficients=[1.0,0.025,-4.0,-0.012,1.0,2.0,0.00015,-0.0001]
records=[]
for camera in [2414,2418,2883,5025,1425,5495]:
    path=source/f'{camera}-base.png';image=Image.open(path).convert('RGB')
    destination=output/f'{camera}-perspective.png'
    image.transform(image.size,Image.Transform.PERSPECTIVE,coefficients,Image.Resampling.BILINEAR).save(destination)
    records.append({'id':camera,'source':str(path.relative_to(root)),'source_sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'file':destination.name,'sha256':hashlib.sha256(destination.read_bytes()).hexdigest(),'size':image.size})
(output/'manifest.json').write_text(json.dumps({'scope':'Authored perspective derivatives of existing pinned real camera JPEG previews, not captured perspective photo pairs. Pillow output-to-input rational mapping; independent encoder/resampler, no rrrah geometry used.','pillow_version':Image.__version__,'inverse_mapping':coefficients,'resampling':'BILINEAR','cases':records},indent=2)+'\n')
