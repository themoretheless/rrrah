from pathlib import Path
import re,json,hashlib
from PIL import Image
root=Path('.dedup-work/ish388-20261021');folder=Path(__file__).resolve().parent
readme=(root/'README.md').read_text();expected=dict(re.findall(r'(testdata/\S+) <\w+>: ([0-9a-f]+)',readme));actual=dict(re.findall(r'(testdata/\S+) <\w+>: ([0-9a-f]+)',(root/'review-example.log').read_text()));assert len(expected)==len(actual)==9;assert expected==actual
rows=[]
for path,h in actual.items():
 p=root/path
 with Image.open(p) as im:rows.append(dict(path=path,format=im.format,mode=im.mode,width=im.width,height=im.height,hash=h,sha256=hashlib.sha256(p.read_bytes()).hexdigest()))
(folder/'result.json').write_text(json.dumps({'example_exit_code':0,'readme_hashes_matched':9,'images':rows,'scope':'Original example with locally recorded module dependencies matches all nine README outputs. Pillow structural image modes/dimensions supplementary to actual visual review, not replacing it. Not independent accuracy/performance qualification.'},indent=2)+'\n')
print(json.dumps(rows,indent=2))
