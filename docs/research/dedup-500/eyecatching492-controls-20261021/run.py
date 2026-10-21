import sys,tempfile,json
from pathlib import Path
sys.path[:0]=[str(Path('.dedup-work/eyecatching492-20261021').resolve()),str(Path('.dedup-work/mediahash478-extra-deps-20261021').resolve())]
from PIL import Image
from eyecatchingutil import MetaImage,ImageComparator
out={}
with tempfile.TemporaryDirectory(prefix='rrrah-eye492-') as tmp:
 root=Path(tmp);black=root/'black.png';Image.new('RGB',(8,8),'black').save(black)
 try:MetaImage(str(black))
 except TypeError as e:out['black_image']={'exception':type(e).__name__,'message':str(e)}
 else:raise AssertionError('expectedblackbboxfailure')
 square=root/'square.png';Image.new('RGB',(8,8),'white').save(square);m=MetaImage(str(square));assert m.first_half() is None and m.second_half() is None
 out['square_halves']={'first':None,'second':None};m.image.close()
 dotted=root/'a.b.png';Image.new('RGB',(8,8),'white').save(dotted);m=MetaImage(str(dotted));assert m.ext=='b';out['multiple_dot_extension']={'reported':m.ext,'actual':'png'};m.image.close()
 try:ImageComparator(Image.new('RGB',(1,1),'red'),Image.new('RGB',(1,1),'red')).is_similar_by_color()
 except IndexError as e:out['one_pixel_colour']={'exception':type(e).__name__}
 else:raise AssertionError('expectedcoordinateerror')
 c=ImageComparator(Image.new('RGB',(16,16),'red'),Image.new('RGB',(16,16),'blue'));dist={alg:int(c.hamming_diff(alg)) for alg in ['ahash','phash','dhash','whash']};assert all(v==0 for v in dist.values());out['different_solid_colours']={'distances':dist,'actual_rgb_equal':False}
out['scope']='Five controls on unchanged original utility module with actual Pillow/ImageHash/NumPy/SciPy/PyWavelets; no browser/screenshots/ImageMagick backend or upstream tests.'
Path(__file__).with_name('result.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
