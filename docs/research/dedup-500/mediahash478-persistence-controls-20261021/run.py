from pathlib import Path
import sys,json,importlib.metadata
root=Path.cwd();sys.path[:0]=[str(root/'.dedup-work/mediahash478-extra-deps-20261021'),'/Users/themoretheless/.codex/tmp/rrrah-mediahash-deps-20261013',str(root/'.dedup-work/mediahash478-20261021/src')]
from mediahash import image,video
from imagehash import ImageHash
import numpy as np
baseline=image.ImageFingerprint(ImageHash(np.zeros((8,8),dtype=bool)));parsed_baseline=image.parse_fingerprint(bytes(baseline))
for bit in range(64):
 data=np.zeros((8,8),dtype=bool);data.flat[bit]=True;original=image.ImageFingerprint(ImageHash(data));parsed=image.parse_fingerprint(bytes(original));assert len(bytes(original))==64;assert bytes(parsed)==bytes(original);assert original-baseline==parsed-parsed_baseline==original-parsed_baseline==parsed-baseline==1
print('PASS64 one-hot image descriptors preserve bytes and Hamming distance for fresh/parsed pairings despite8x8 versus64 shape')
source=root/'.dedup-work/mediahash478-20261021/__test__/data/sample.mp4'
with source.open('rb') as f:fresh=video.fingerprint(f)
parsed=video.parse_fingerprint(bytes(fresh));assert bytes(parsed)==bytes(fresh)
try:fresh-parsed
except ValueError as e:assert '1-D' in str(e)
else:raise AssertionError('expected fresh/parsed comparison shape failure')
assert parsed-parsed==0.0
print('PASS actual decoded video fresh/parsed pair raises1D ValueError while parsed self-distance0; bytes preserved')
print(json.dumps({'new_authored_controls':2,'image_basis_cases':64,'fresh_video_shape':list(fresh._fp.shape),'parsed_video_shape':list(parsed._fp.shape),'packages':{n:importlib.metadata.version(n) for n in ['av','ImageHash','numpy','scipy']}}))
