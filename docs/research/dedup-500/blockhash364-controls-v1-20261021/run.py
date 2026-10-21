"""Unchanged original blockhash functions with authored Pillow images only."""
from pathlib import Path
import sys
sys.path.insert(0,str(Path('.dedup-work/blockhash364-20261021').resolve()))
import blockhash as b
from PIL import Image
assert b.median([7,1,3])==3 and b.median([4,2])==3
print('PASS median odd/even known values')
a=Image.new('RGB',(17,17),'black');z=a.copy()
for y in range(17):z.putpixel((16,y),(255,255,255))
for x in range(17):z.putpixel((x,16),(255,255,255))
assert b.blockhash_even(a,16)==b.blockhash_even(z,16)
assert b.blockhash(a,16)!=b.blockhash(z,16)
print('PASS quick17x17 ignores last row/column; weighted path detects fixture border')
a=Image.new('RGBA',(16,16),(255,0,0,1));z=Image.new('RGBA',(16,16),(255,0,0,255))
assert b.blockhash(a,16)==b.blockhash(z,16)
print('PASS sameRGB alpha1/255 hash collision')
a=Image.new('RGBA',(16,16),(0,0,0,0));z=Image.new('RGBA',(16,16),(255,0,0,0))
assert b.blockhash(a,16)==b.blockhash(z,16)
print('PASS fully transparent hiddenRGB canonical')
try:b.blockhash(Image.new('RGB',(16,16)),0)
except ZeroDivisionError:print('PASS zero bits raises ZeroDivisionError')
else:raise AssertionError('expected zero bits failure')
