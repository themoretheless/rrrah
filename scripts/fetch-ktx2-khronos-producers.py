"""Fetch pinned Apache-2.0 official KTX2 producers and extract plain sample contracts.
This independent byte reader only handles the two fixed SHA256-verified RGB/RGBA files.
"""
from pathlib import Path
import urllib.request,struct,hashlib
ROOT=Path(__file__).resolve().parent.parent/'tests/fixtures/raster'
REVISION='e3c91805e874bcab9447e8689afa5bc39266bada'
SOURCES={'r8g8b8a8_srgb.ktx2': {'sha256': 'e49f639e1af5e557e03b630ae07b8090c830924fabba3e0ee13c083cf552eeaf', 'bytes': 65800}, 'r8g8b8_srgb_mip.ktx2': {'sha256': 'da5c0dd90920620dc3634ff0842c775c36e86c4477ffde29607c9e3fa536bd91', 'bytes': 16788}}
def main():
 lines=['# source\tmip\twidth\theight\tframes\toracle\tsha256']
 for name,info in SOURCES.items():
  url=f'https://media.githubusercontent.com/media/KhronosGroup/KTX-Software/{REVISION}/tests/resources/ktx2/{name}'
  b=urllib.request.urlopen(url).read()
  assert len(b)==info['bytes'] and hashlib.sha256(b).hexdigest()==info['sha256']
  target='khronos-'+name;(ROOT/target).write_bytes(b)
  vk,size,w,h,depth,layer,face,levels,compression=struct.unpack_from('<9I',b,12)
  assert size==1 and vk in(29,43) and (depth,layer,face,compression)==(0,0,1,0)
  channels=3 if vk==29 else 4
  for level in range(levels):
   width,height=max(1,w>>level),max(1,h>>level)
   at,n,raw=struct.unpack_from('<3Q',b,80+24*level);assert n==raw==width*height*channels
   packed=b[at:at+n];rgba=packed if channels==4 else b''.join(packed[i:i+3]+b'\xff' for i in range(0,len(packed),3))
   oracle=f'{target}.mip{level}.rgba';(ROOT/oracle).write_bytes(rgba)
   lines.append(f'{target}\t{level}\t{width}\t{height}\t{levels}\t{oracle}\t{info["sha256"]}')
 (ROOT/'ktx2-khronos-producer-manifest.tsv').write_text('\n'.join(lines)+'\n')
if __name__=='__main__':main()
