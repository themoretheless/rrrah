#!/usr/bin/env python3
"""Independent Pillow encoder fixtures for native decoder regression tests."""
import argparse,hashlib,json
from pathlib import Path
from PIL import Image,__version__
def main():
 ap=argparse.ArgumentParser();ap.add_argument('output',type=Path);args=ap.parse_args();p=args.output;assert not p.exists();p.mkdir(parents=True)
 a=Image.new('P',(9,9),0);b=Image.new('P',(9,9),0);palette=[v for i in range(256)for v in (i,i,i)]
 for im in [a,b]:im.putpalette(palette);im.info['transparency']=bytes(256)
 b.putdata([x*20 for y in range(9)for x in range(9)]);a.save(p/'hidden-black.png');b.save(p/'hidden-gradient.png');Image.new('RGBA',(9,9),(255,0,255,0)).save(p/'hidden-rgba.png');c=b.copy();alpha=bytearray(256);alpha[180]=255;c.info['transparency']=bytes(alpha);c.putpixel((4,4),180);c.save(p/'visible-center.png')
 for color in ['black','white']:Image.new('RGB',(9,9),color).save(p/(color+'.png'))
 print(json.dumps({'pillow':__version__,'files':{f.name:hashlib.sha256(f.read_bytes()).hexdigest()for f in sorted(p.iterdir())}},indent=2))
if __name__=='__main__':main()
