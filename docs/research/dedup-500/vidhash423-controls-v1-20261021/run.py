"""Original match/video modules; authored scalar frame hashes and options."""
from pathlib import Path
import importlib.util,sys,types
from dataclasses import dataclass
R=Path(__file__).resolve().parents[4];D=R/'docs/research/dedup-500/evidence/SpangleLabs--vidhash';C=R/'.dedup-work/vidhash423-20261021'
sys.modules['vidhash']=types.ModuleType('vidhash')
def load(n,p):
 spec=importlib.util.spec_from_file_location('vidhash.'+n,p);m=importlib.util.module_from_spec(spec);sys.modules[spec.name]=m;spec.loader.exec_module(m);return m
m=load('match_options',D/'04-match_options.py');v=load('video_hash',C/'vidhash/video_hash.py')
@dataclass(frozen=True)
class Frame:
 n:int
 def similar_to(self,other,dist):return abs(self.n-other.n)<=dist
opts=types.SimpleNamespace(fps=1,settings=types.SimpleNamespace(blank_hash=Frame(-100)))
def video(xs):return v.VideoHash([Frame(x) for x in xs],len(xs),opts)
a=video([10,20,30]);b=video([30,20,10])
assert m.PercentageMatch(hamming_dist=0,percentage_overlap=100).check_match(a,b)
print('PASS original100percent overlap accepts reversed sequence (unordered overlap policy)')
assert not m.DurationMatch(hamming_dist=0,time_overlap=3).check_match(a,b)
print('PASS original duration positive-order control rejects same reversed sequence')
assert m.DurationMatch(hamming_dist=0,time_overlap=3).check_match(video([10]),video([10]))
print('PASS requested3second duration is clamped to available one-frame match')
c=video([10]);c.hash_options=object()
try:m.PercentageMatch().check_match(a,c)
except m.MatchException:print('PASS differing hash options explicitly rejected')
else:raise AssertionError('Expected incompatible options failure')
assert not m.PercentageMatch().check_match(video([]),video([]))
assert not m.DurationMatch().check_match(video([]),video([]))
print('PASS empty sequences do not match under percentage or duration')
print('5 original-module controls passed; no imagehash, video decoding, FFmpeg or actual timestamps')
