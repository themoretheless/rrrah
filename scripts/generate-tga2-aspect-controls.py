"""TGA2 rational pixel aspect; reuse alpha3 pixels and verify OIIO metadata."""
from pathlib import Path
import OpenImageIO as oiio
import struct,hashlib,json
root=Path(__file__).resolve().parents[1]/'tests/fixtures/raster'
alpha=json.loads((root/'tga2-alpha-reference.json').read_text());rows=[]
for original in alpha['rows']:
 if original['alpha_type']!=3:continue
 source=(root/original['name']).read_bytes();assert hashlib.sha256(source).hexdigest()==original['source_sha256']
 ext=struct.unpack_from('<I',source,len(source)-26)[0]
 for numerator,denominator in [(2,1),(1,2),(7,7),(0,0)]:
  data=bytearray(source);struct.pack_into('<HH',data,ext+474,numerator,denominator)
  path=root/original['name'].replace('alpha-mode3',f'aspect-{numerator}-{denominator}');path.write_bytes(data)
  config=oiio.ImageSpec();config.attribute('oiio:UnassociatedAlpha',1);inp=oiio.ImageInput.open(str(path),config);assert inp,oiio.geterror()
  actual=inp.spec().get_float_attribute('PixelAspectRatio',1.);expected=numerator/denominator if denominator else None
  assert actual==(expected if expected is not None else 1.)
  pixels=inp.read_image(format=oiio.UINT8).tobytes();inp.close();oracle=(root/original['rgba8_file']).read_bytes();assert pixels==oracle
  rows.append({'name':path.name,'source_sha256':hashlib.sha256(data).hexdigest(),'width':original['width'],'height':original['height'],'rgba8_file':original['rgba8_file'],'rgba8_sha256':original['rgba8_sha256'],'alpha_type':3,'pixel_aspect':expected,'rational':[numerator,denominator],'oiio_effective_aspect':actual})
(root/'tga2-aspect-reference.json').write_text(json.dumps({'scope':'Authored TGA2 mode3 samples, all4 origins/raw-RLE; rational width/height pixel aspect2,0.5,1 or unspecified. Independent unmodified OpenImageIO complete samples and effective aspect verified. Existing pixel oracles reused.','openimageio':oiio.VERSION_STRING,'rows':rows},indent=2)+'\n')
print('Generated',len(rows),'TGA2 aspect sources; all OIIO samples and ratios exact.')
