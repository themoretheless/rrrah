#!/usr/bin/env python3
from pathlib import Path
import struct,json,hashlib,argparse
from PIL import Image,ImageOps
parser=argparse.ArgumentParser(description='Attach CMYK ICC/EXIF and independently orient LittleCMS reference pixels')
parser.add_argument('--input-dir',type=Path,required=True,help='cmyk/ycck-asymmetric.jpg and .rgbf32le produced by external codecs and LittleCMS')
parser.add_argument('--profile',type=Path,required=True)
parser.add_argument('--output-dir',type=Path,required=True)
args=parser.parse_args()
root=args.output_dir;root.mkdir(parents=True,exist_ok=True);profile=args.profile.read_bytes();icc=b'ICC_PROFILE\0'+bytes([1,1])+profile
records=[]
for kind in ['cmyk','ycck']:
 source=(args.input_dir/(kind+'-asymmetric.jpg')).read_bytes()
 floats=struct.unpack('<663f',(args.input_dir/(kind+'-asymmetric.rgbf32le')).read_bytes())
 rgb=bytes(max(0,min(255,int(v*255+.5))) for v in floats)
 for orientation in range(1,9):
  exif=b'Exif\0\0'+b'II'+struct.pack('<HIH',42,8,1)+struct.pack('<HHIHH',274,3,1,orientation,0)+struct.pack('<I',0)
  data=source[:2]+b'\xff\xe1'+(len(exif)+2).to_bytes(2,'big')+exif+b'\xff\xe2'+(len(icc)+2).to_bytes(2,'big')+icc+source[2:]
  name=f'{kind}-profiled-orientation-{orientation}'
  (root/(name+'.jpg')).write_bytes(data)
  golden=Image.frombytes('RGB',(17,13),rgb);golden.getexif()[274]=orientation
  golden=ImageOps.exif_transpose(golden).convert('RGBA');ref=golden.tobytes();(root/(name+'.rgba')).write_bytes(ref)
  records.append({'name':name,'orientation':orientation,'size':golden.size,'source_sha256':hashlib.sha256(data).hexdigest(),'reference_sha256':hashlib.sha256(ref).hexdigest()})
(root/'profiled-jpeg-orientation-reference.json').write_text(json.dumps({'producer':'Pillow/libjpeg CMYK and libjpeg-turbo YCCK; independent libjpeg CMYK decode, LittleCMS float ICC and Pillow EXIF transpose','cases':records},indent=2)+'\n')
