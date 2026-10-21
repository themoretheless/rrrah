import sys,json,io,struct,zlib
from pathlib import Path
sys.path[:0]=[str(Path('.dedup-work/mediadedupe444-20261021').resolve()),str(Path('.dedup-work/mediahash478-extra-deps-20261021').resolve())]
import engine
from PIL import Image
zero='0000000000000000';far='ffffffffffffffff'
def row(name,hashes):return dict(path=name,intensive_start=hashes,intensive_end=hashes)
a=row('A',[zero]);b=row('B',[zero]+[far]*9)
x=engine.find_intensive_video_duplicates([a,b],set(),threshold=0)
y=engine.find_intensive_video_duplicates([b,a],set(),threshold=0)
assert len(x)==1 and y==[]
out={'intensive_directionality':dict(AB_groups=len(x),BA_groups=len(y),A_frames=1,B_frames=10,threshold=0,min_fraction=0.3,scope='Actual original comparator, supplied hashes; no video decoder invoked')}
buf=io.BytesIO();Image.new('RGB',(2,2),'red').save(buf,format='PNG');png=buf.getvalue();payload=b'note\0prefix\x89PNG\r\n\x1a\nsuffix';chunk=b'tEXt'+payload;insert=struct.pack('>I',len(payload))+chunk+struct.pack('>I',zlib.crc32(chunk)&0xffffffff);png=png[:33]+insert+png[33:]
im=Image.open(io.BytesIO(png));im.load();assert im.size==(2,2)
parts=engine._split_png_stream(png);assert len(parts)==2
out['valid_png_embedded_signature']=dict(valid_images=1,split_fragments=len(parts),fragment_lengths=list(map(len,parts)),scope='Real Pillow-decoded valid PNG with ancillary text chunk; original split helper only, no ffmpeg output claim')
try:engine.find_visual_video_duplicates([dict(path='bad',duration=1,frame_hashes='{broken')],set())
except json.JSONDecodeError:out['malformed_cached_frames']={'result':'JSONDecodeError escapes whole operation'}
else:raise AssertionError('expected decode error')
Path(__file__).with_name('result.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
