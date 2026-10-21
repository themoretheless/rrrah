"""Original customSimhash module; synthetic integer fingerprints, no corpus."""
from pathlib import Path
import importlib.util,logging
R=Path(__file__).resolve().parents[4]
p=R/'docs/research/dedup-500/evidence/aplmikex--deduplication_mnbvc/04-customSimhash.py'
spec=importlib.util.spec_from_file_location('original_customSimhash',p);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
logging.disable(logging.CRITICAL)
for radius in range(4):
 idx=m.SimhashIndex([(str(x),m.Simhash(x,f=8)) for x in range(256)],f=8,k=radius)
 for q in range(256):
  actual=set(idx.get_near_dups(m.Simhash(q,f=8)))
  expected={str(x) for x in range(256) if (x^q).bit_count()<=radius}
  assert actual==expected,(radius,q)
print('PASS all256 queries against all2568bit values at radii0..3 match independent exhaustive oracle')
idx=m.SimhashIndex([(str(i),m.Simhash(0)) for i in range(60)],k=0)
assert set(idx.get_near_dups(m.Simhash(0)))=={str(i) for i in range(60)}
print('PASS all60 tied IDs retained by plural search')
for bit in range(64):assert m.Simhash(0).distance(m.Simhash(1<<bit))==1
print('PASS all64 one-hot bit distances')
assert m.Simhash(0,f=8)==m.Simhash(0,f=16)
assert m.Simhash(0,f=8).distance(m.Simhash(256,f=8))==0
print('PASS unbounded integer input high bits ignored by distance and equality ignores fingerprint width')
print('4 controls passed; original NumPy dependency loaded, no text corpus, queues, pandas or filesystem processing')
