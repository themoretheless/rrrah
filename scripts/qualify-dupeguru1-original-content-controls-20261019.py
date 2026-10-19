"""Execute pinned original engine/fs modules on task-owned ordinary synthetic files."""
from pathlib import Path
import hashlib
import json
import os
import subprocess
import sys

R = Path(__file__).resolve().parents[1]
S = R / '.dedup-work/dupeguru1-20261019/source'
W = R / '.dedup-work/dupeguru1-content-controls-v2-20261019'
H = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()

def main():
    assert subprocess.check_output(['git','-C',str(S),'rev-parse','HEAD'],text=True).strip()=='db097a030150b3d4ea9f492b85e7e4e4bcb3f789'
    W.mkdir(exist_ok=False)
    pins={p:H(S/p) for p in ['core/engine.py','core/fs.py']}
    for p in pins:
        assert subprocess.check_output(['git','-C',str(S),'show','HEAD:'+p])==(S/p).read_bytes()
    sys.path.insert(0,str(S))
    from core import engine, fs
    fs.filesdb.connect(W/'cache.sqlite')
    files=W/'files'; files.mkdir()
    a=files/'original.bin'; b=files/'copy.bin'; c=files/'different.bin'
    payload=b'\x25'*(8*1024*1024)
    a.write_bytes(payload); b.write_bytes(payload)
    changed=bytearray(payload); changed[len(payload)//2]^=1; c.write_bytes(changed)
    def wrapped(paths):
        result=[fs.File(p) for p in paths]
        for item in result:item.is_ref=False
        return result
    def pairs(matches):
        return sorted(sorted([m.first.name,m.second.name]) for m in matches)
    full=engine.getmatches_by_contents(wrapped([a,b,c]),bigsize=0)
    sampled=engine.getmatches_by_contents(wrapped([a,b,c]),bigsize=fs.MIN_FILE_SIZE)
    assert pairs(full)==[['copy.bin','original.bin']]
    assert len(sampled)==3 and all(m.percentage==100 for m in sampled)
    originals=wrapped([a,c]); first,second=originals
    assert first.digest_partial==second.digest_partial
    assert first.digest_samples==second.digest_samples
    assert first.digest!=second.digest and H(a)!=H(c)
    stale=files/'cache-rewrite.bin'; stale.write_bytes(payload)
    before_stat=stale.stat(); before_digest=fs.File(stale).digest
    stale.write_bytes(bytes(changed)); os.utime(stale,ns=(before_stat.st_atime_ns,before_stat.st_mtime_ns))
    after_digest=fs.File(stale).digest
    assert stale.stat().st_mtime_ns==before_stat.st_mtime_ns and H(stale)!=H(a)
    assert before_digest==after_digest
    fs.filesdb.close()
    assert all(H(S/p)==pin for p,pin in pins.items())
    result={'status':'reproduced_sample_only_acceptance_and_restored_timestamp_cache_reuse','source_pins':pins,'backend':fs.hasher.__module__+'.'+fs.hasher.__name__,'modules':{'engine':engine.__file__,'fs':fs.__file__},'synthetic_size':len(payload),'different_byte_offset':len(payload)//2,'full_content_pairs':pairs(full),'sample_only_pairs':pairs(sampled),'sample_only_percentages':[m.percentage for m in sampled],'restored_timestamp_cache_reuse':{'mtime_ns_preserved':True,'content_changed_sha256':True,'fresh_File_digest_equals_cached_old_digest':True},'input_sha256':{p.name:H(p) for p in [a,b,c,stale]},'limits':['Original complete modules imported, no mocked hash/digest or extracted functions.','Absent optional xxhash selects upstream-supported hashlib.md5 fallback; no xxHash runtime claim.','Synthetic ordinary file controls only; no GUI, whole upstream tests, deletion action or full repository claim.','This demonstrates sampled coverage and cache-key limitations, not a cryptographic hash collision.']}
    (W/'result.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result))

if __name__=='__main__':main()
