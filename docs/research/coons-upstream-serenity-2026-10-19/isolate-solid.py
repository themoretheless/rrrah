#!/usr/bin/env python3
"""Replace only source mesh color codes with zero, preserving all byte offsets."""
from pathlib import Path
import argparse,json,hashlib,re,string
p=Path(__file__).resolve().parent
ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('corpus',type=Path);a=ap.parse_args()
manifest=json.loads((p/'isolated-sources.json').read_text());rows=[]
for source in manifest['sources']:
 raw=(a.corpus/source['name']).read_bytes();assert hashlib.sha256(raw).hexdigest()==source['sha256']
 out=bytearray(raw)
 for match in re.finditer(rb'<<(.*?)>>\s*stream\r?\n(.*?)\r?\nendstream',raw,re.S):
  header,body=match.group(1,2)
  if b'/ASCIIHexDecode' not in header:continue
  count=12 if b'/ShadingType 6' in header else 16
  components=1 if b'/Function ' in header else 3
  positions=[i for i,c in enumerate(body) if chr(c) in string.hexdigits]
  data=bytes.fromhex(bytes(body[i] for i in positions).decode());offset=0
  while offset<len(data):
   flag=data[offset];assert flag in (0,1,2,3)
   offset+=1+(count if flag==0 else count-4)*4
   end=offset+(4 if flag==0 else 2)*components
   assert end<=len(data)
   for i in range(offset,end):
    for digit in [0,1]:out[match.start(2)+positions[2*i+digit]]=ord('0')
   offset=end
  assert offset==len(data)
 name=source['name'].replace('.pdf','-solid.pdf');(a.corpus/name).write_bytes(out)
 rows.append(dict(name=name,source=source['source'],parent_sha256=source['sha256'],transformation='same-length all mesh source component codes zero; selected paint unchanged',bytes=len(out),sha256=hashlib.sha256(out).hexdigest()))
(p/'solid-sources.json').write_text(json.dumps(dict(revision=manifest['revision'],sources=rows),indent=2)+'\n')
