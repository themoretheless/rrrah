"""Original getmatches with real C blocks, Pillow decode adapter and two real workers."""
from pathlib import Path
import hashlib
import json
import multiprocessing
from multiprocessing.pool import Pool as RealPool
import shutil
import sys

R=Path(__file__).resolve().parents[1]
S=R/'.dedup-work/dupeguru1-20261019/source'
W=R/'.dedup-work/dupeguru1-picture-pipeline-20261019'
sys.path.insert(0,str(S))
from PIL import Image
from core import fs
from core.pe import block,matchblock
from core.pe.photo import Photo

class PillowPhoto(Photo):
    def _plat_get_dimensions(self):
        with Image.open(self.path)as image:return image.size
    def _plat_get_blocks(self,count,orientation):
        assert orientation in [0,1]
        with Image.open(self.path)as image:return block.getblocks2(image.convert('RGB'),count)

def main():
    W.mkdir(exist_ok=False)
    a=Image.new('RGB',(30,30));b=Image.new('RGB',(30,30))
    for y in range(30):
        for x in range(30):
            v=255*((x+y)%2);a.putpixel((x,y),(v,v,v))
            v=255*(x%2);b.putpixel((x,y),(v,v,v))
    a.save(W/'checker.png');b.save(W/'stripes.png');shutil.copyfile(W/'checker.png',W/'copy.png')
    fs.filesdb.connect(W/'hashes.sqlite')
    original_pool=multiprocessing.Pool
    multiprocessing.Pool=lambda *args,**kwargs:RealPool(processes=2)
    try:
        def run(threshold):
            pictures=[PillowPhoto(W/name)for name in ['checker.png','copy.png','stripes.png']]
            for picture in pictures:picture.is_ref=False
            matches=matchblock.getmatches(pictures,str(W/'pictures.sqlite'),threshold=threshold)
            return sorted([sorted([m.first.name,m.second.name]),m.percentage]for m in matches)
        strict=run(100);fuzzy=run(99)
    finally:
        multiprocessing.Pool=original_pool;fs.filesdb.close()
    assert strict==[[['checker.png','copy.png'],100]],strict
    assert fuzzy==[[['checker.png','copy.png'],100],[['checker.png','stripes.png'],99],[['copy.png','stripes.png'],99]],fuzzy
    h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
    result={'status':'verified_original_picture_getmatches_digest_downgrade','threshold100_matches':strict,'threshold99_matches':fuzzy,'source_pins':{p:h(S/p)for p in ['core/pe/matchblock.py','core/pe/photo.py','core/fs.py']},'input_sha256':{name:h(W/name)for name in ['checker.png','copy.png','stripes.png']},'adapters':['Photo platform methods implemented with real Pillow decode and original C getblocks2 for unrotated normal RGB PNGs.','Only pool factory capped at two real multiprocessing workers; original matching/candidate/final-digest functions unchanged.'],'limits':['Not native Qt/Cocoa decoding, GUI or unrestricted upstream default process count.','99percent is a fuzzy candidate outcome, not exact pixel identity.','No invalid buffers, memory-error probe, production source modification or performance claim.']}
    (W/'result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))

if __name__=='__main__':main()
