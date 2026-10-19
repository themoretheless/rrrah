"""Normal RGB fixtures through complete original C block and SQLite candidate modules."""
from pathlib import Path
import hashlib
import json
import os
import subprocess
import sys

R=Path(__file__).resolve().parents[1]
S=R/'.dedup-work/dupeguru1-20261019/source'
W=R/'.dedup-work/dupeguru1-native-picture-controls-20261019'
H=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()

def main():
    assert subprocess.check_output(['git','-C',str(S),'rev-parse','HEAD'],text=True).strip()=='db097a030150b3d4ea9f492b85e7e4e4bcb3f789'
    W.mkdir(exist_ok=False)
    sys.path.insert(0,str(S))
    from PIL import Image
    from core.pe import block,matchblock,_block,_cache
    from core.pe.cache_sqlite import SqliteCache
    native={m.__file__:H(Path(m.__file__))for m in [_block,_cache]}
    source={p:H(S/p)for p in ['core/pe/matchblock.py','core/pe/cache_sqlite.py','core/pe/cache.py']}
    a=Image.new('RGB',(30,30));b=Image.new('RGB',(30,30))
    for y in range(30):
        for x in range(30):
            v=255*((x+y)%2);a.putpixel((x,y),(v,v,v))
            v=255*(x%2);b.putpixel((x,y),(v,v,v))
    ap=W/'checker.png';bp=W/'stripes.png';a.save(ap);b.save(bp)
    ab=block.getblocks2(a,15);bb=block.getblocks2(b,15)
    assert len(ab)==len(bb)==225 and ab==bb==[(127,127,127)]*225
    assert a.tobytes()!=b.tobytes() and H(ap)!=H(bp)
    assert block.avgdiff(ab,bb,0,3)==0
    sheet=Image.new('RGB',(360,180));sheet.paste(a.resize((180,180),Image.Resampling.NEAREST),(0,0));sheet.paste(b.resize((180,180),Image.Resampling.NEAREST),(180,0));sheet.save(W/'fixtures.png')
    cache=SqliteCache(str(W/'blocks.sqlite'))
    cache[str(ap)]=[ab]+[[]for _ in range(7)];cache[str(bp)]=[bb]+[[]for _ in range(7)]
    aid=cache.get_id(str(ap));bid=cache.get_id(str(bp));cache.close()
    candidate=matchblock.async_compare([aid],[bid],str(W/'blocks.sqlite'),100,{aid:((30,30),False),bid:((30,30),False)})
    assert candidate==[(aid,bid,100)]
    stamp=1700000000*1000000000
    os.utime(ap,ns=(stamp+100,stamp+100));cache=SqliteCache(str(W/'mtime.sqlite'));cache[str(ap)]=[ab]+[[]for _ in range(7)]
    red=Image.new('RGB',(30,30),(255,0,0));red.save(ap);os.utime(ap,ns=(stamp+900000000,stamp+900000000))
    cache.purge_outdated();retained=str(ap)in cache;cached=cache[str(ap)][0];cache.close()
    assert retained and cached==ab and block.getblocks2(red,15)!=cached
    assert all(H(Path(p))==pin for p,pin in native.items()) and all(H(S/p)==pin for p,pin in source.items())
    result={'status':'verified_original_native_block_candidates_and_same_second_cache_reuse','native_binary_pins':native,'source_pins':source,'real_RGB_fixture_dimensions':[30,30],'block_count':225,'different_pixels_same_block_means':True,'original_async_candidate_score':candidate[0][2],'same_second_picture_cache_retains_old_blocks':retained,'fixture_contact_sheet_sha256':H(W/'fixtures.png'),'limits':['Original complete C modules with normal Pillow RGB images; no invalid native inputs, sanitizer or memory-error probes.','async_compare is candidate stage. Original getmatches final digest check may lower100to99; full multiprocessing/publication pipeline not invoked here.','Qt/Cocoa platform decode paths not qualified by Pillow fixture calls.','Cache observation is same-second timestamp granularity, not a hash collision. No production code adoption.']}
    (W/'result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))

if __name__=='__main__':main()
