from pathlib import Path
import sys,json,subprocess
root=Path.cwd();sys.path[:0]=[str(root/'.dedup-work/mtg500-deps-20261021'),str(root/'.dedup-work/mediahash478-pytest-20261021'),str(root/'.dedup-work/mtg500-20261021')]
import cv2,numpy as np,pytest
print(json.dumps({'opencv':cv2.__version__,'numpy':np.__version__,'pytest':pytest.__version__}),flush=True)
rc=pytest.main(['-q','-p','no:cacheprovider',str(root/'.dedup-work/mtg500-20261021/tests/test_image_hash.py'),str(root/'.dedup-work/mtg500-20261021/tests/test_embed_golden.py')]);print('ORIGINAL_PYTEST_EXIT',rc,flush=True)
from image_hash import HASH_BYTES,hash_card_art,hamming_distances,min_distances
from embed import quantize
red=np.zeros((1040,745,3),dtype=np.uint8);red[:,:,2]=255;blue=np.zeros_like(red);blue[:,:,0]=255
assert hash_card_art(red)==hash_card_art(blue)==bytes(HASH_BYTES)
print('PASS different solidBGR colours share zero1024bit dHash')
mat=np.zeros((2,HASH_BYTES),dtype=np.uint8);assert hamming_distances(bytes(8),mat).tolist()==[0,0]
print('PASS8byte query broadcasts across128byte hash rows without rejection')
assert min_distances([],mat) is None
print('PASS noquery min_distances returnsNone rather than typed ndarray')
import warnings
with warnings.catch_warnings(record=True) as w:
 warnings.simplefilter('always');out=quantize(np.full(384,np.nan,dtype=np.float32))
assert out==bytes(384);assert w
print('PASS nonfiniteembedding quantizes384zero bytes withwarning butno exception')
print(json.dumps({'new_authored_controls':4,'original_test_exit':int(rc)}));sys.exit(int(rc))
