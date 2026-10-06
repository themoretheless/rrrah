"""CC0 authored crop, independently encoded by Pillow from pinned base pixels."""
from pathlib import Path
from PIL import Image, __version__
import hashlib, json
repo=Path(__file__).resolve().parent.parent
base=repo/'crates/rrrah-dedup/tests/fixtures/rotation/base.png'
root=repo/'crates/rrrah-dedup/tests/fixtures/local-files';root.mkdir(parents=True,exist_ok=True)
Image.open(base).crop((20,20,140,140)).save(root/'crop.png')
(root/'manifest.json').write_text(json.dumps({'pillow':__version__,'license':'CC0-1.0','source_sha256':hashlib.sha256(base.read_bytes()).hexdigest(),'crop':[20,20,140,140],'sha256':{'crop.png':hashlib.sha256((root/'crop.png').read_bytes()).hexdigest()}},indent=2)+'\n')
