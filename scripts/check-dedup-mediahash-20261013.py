"""Execute unchanged pinned mediahash with real dependencies, not mocked decoders."""
import sys, pathlib, json, importlib.metadata, hashlib, warnings
T=pathlib.Path('/Users/themoretheless/.codex/tmp')
S=T/'rrrah-mediahash-source-20261013'
sys.path[:0]=[str(T/'rrrah-mediahash-deps-20261013'),str(S/'src')]
from mediahash import image,video,util
from PIL import Image as PILImage
import numpy as np
from imagehash import ImageHash
rows=[]
for name in ['analyze','checksum','fingerprint']:
 with (S/'__test__/data/frame.jpg').open('rb') as f:
  try: getattr(image,name)(f)
  except AttributeError as e:
   assert 'open' in str(e);rows.append(dict(control='image_'+name,result='AttributeError',message=str(e)))
  else: raise AssertionError('expected actual global Image shadow failure')
with (S/'__test__/data/sample.mp4').open('rb') as f:
 info=video.analyze(f)
 assert (info.width,info.height,info.length)==(480,270,3)
 rows.append(dict(control='video_analyze_real_fixture',result=vars(info)))
with (S/'__test__/data/sample.mp4').open('rb') as f:
 fp=video.fingerprint(f)
 assert fp._fp.shape==(8,8)
 try: fp-fp
 except ValueError as e:
  assert '1-D' in str(e);rows.append(dict(control='fresh_video_self_distance',result='ValueError',message=str(e)))
 else: raise AssertionError('unexpected multidimensional cosine acceptance')
 parsed=video.parse_fingerprint(bytes(fp))
 assert parsed._fp.shape==(64,) and parsed-parsed==0.
 rows.append(dict(control='serialized_video_flat_self_distance',result=float(parsed-parsed)))
with (S/'__test__/data/sample.mp4').open('rb') as f:
 try: video.checksum(f)
 except StopIteration:
  rows.append(dict(control='sample_video_checksum',result='StopIteration'))
 else: raise AssertionError('expected short two-chunk fixture refusal')
def item(name,bits):
 return image.Image(name=name,path=pathlib.Path(name),info=None,checksum=name.encode(),fingerprint=image.ImageFingerprint(ImageHash(np.array(bits,dtype=bool))))
a,b,c=item('A',[0,0]),item('B',[0,1]),item('C',[1,1])
assert a.fingerprint-c.fingerprint==2
components=list(util.dedup([a,b,c],1));assert len(components)==1 and len(components[0])==3
rows.append(dict(control='connected_component_chain',result=sorted(i.name for i in components[0]),endpoint_distance=2,threshold=1))
with warnings.catch_warnings():
 warnings.simplefilter('ignore'); value=video.VideoFingerprint(np.zeros(64))-video.VideoFingerprint(np.zeros(64))
 assert np.isnan(value)
rows.append(dict(control='zero_video_self_distance',result='NaN'))
packages={name:importlib.metadata.version(name) for name in ['av','context-utils','typing_extensions','Pillow','ImageHash','numpy','scipy','PyWavelets','networkx']}
P=pathlib.Path('docs/research/dedup-500/mediahash-controls-20261013.json');assert not P.exists()
P.write_text(json.dumps(dict(status='verified_nine_real_dependency_controls',source=str(S),controls=rows,packages=packages,script_sha256=hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest(),scope='Nine controls, unchanged full imported modules and real PyAV/Pillow/numpy/scipy/networkx/context-utils; upstream pytest not run; environment not full lock reconstruction'),indent=2)+'\n')
print(len(rows),'controls passed')
