"""Whole unchanged Python/classes and unchanged .pyx regenerated with Cython3; independent loop oracle over native Murmur primitive."""
from pathlib import Path
import ctypes,hashlib,json,sys
R=Path(__file__).resolve().parents[1];W=R/'.dedup-work/lsh27-20261016';S=W/'source';B=W/'modern-build';D=R/'docs/research/dedup-500';sys.path.insert(0,str(B));import numpy as np
from lsh.minhash import MinHasher
from lsh.cache import Cache
from lsh.cMinhash import minhash_32,minhash_64
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();out=[]
def cap(f):
 try:return dict(value=f())
 except Exception as e:return dict(error=type(e).__name__,message=str(e))
def add(name,value):out.append(dict(control=name,result=value))
# Arrays undergo elementwise &,| then len: no set similarity / coordinate match rate.
m=MinHasher(4,random_state=0);a=np.array([1,2,3,4],dtype=np.uint64);b=np.array([5,6,7,8],dtype=np.uint64);v=m.jaccard(a,b);assert v==1 and not np.any(a==b);add('disjoint_array_fingerprints_report_similarity1',v)
v=cap(lambda:m.jaccard(a,np.array([1,2,3],dtype=np.uint64)));assert v['error']=='ValueError';add('unequal_lengths_broadcast_error',v)
v=cap(lambda:m.jaccard('abcdefgh',a));assert v['error']=='TypeError';add('mixed_string_array_error',v)
x=m.fingerprint('a');y=m.fingerprint('b');assert np.array_equal(x,y) and list(map(int,x))==[2**63-1]*4;add('distinct_short_documents_share_all_sentinel_hashes',list(map(int,x)))
c=Cache(MinHasher(4,random_state=0),num_bands=2);c.add_doc('a',1);assert c.get_duplicates_of('b')=={1};add('whole_cache_false_candidate_for_short_strings',[1])
a=m.fingerprint('abcdefgh');a[0]=0;assert m.fingerprint('abcdefgh') is a and m.fingerprint('abcdefgh')[0]==0;add('cached_fingerprint_mutable_alias',True)
c=Cache(MinHasher(4,random_state=0),num_bands=2);a=np.array([1,2,3,4],dtype=np.uint64);c.add_fingerprint(a,1);a[:]=10;assert np.array_equal(c.fingerprints[1],a);v=cap(lambda:c.remove_id(1));assert v['error']=='KeyError';add('caller_mutation_corrupts_remove_index',v)
c=Cache(MinHasher(4,random_state=0),num_bands=2);a=np.array([1,2,3,4],dtype=np.uint64);b=np.array([5,6,7,8],dtype=np.uint64);c.add_fingerprint(a,1);c.add_fingerprint(b,1);c.add_fingerprint(a,2);assert (1,2) in c.get_all_duplicates(min_jaccard=.99);add('reinsert_id_leaves_old_band_membership_and_false_filtered_pair',[[1,2]])
c.remove_id(1);v=cap(lambda:c.get_all_duplicates(min_jaccard=.5));assert v['error']=='KeyError';add('removed_reinserted_id_still_in_old_bins',v)
c=Cache(MinHasher(4,random_state=0),num_bands=2);c.add_doc('abcdefgh',1);c.clear();assert len(c.fingerprints)==1;v=cap(lambda:c.remove_id(1));assert v['error']=='KeyError';add('clear_retains_fingerprint_and_breaks_remove',v)
v=cap(lambda:Cache(m,num_bands=0));assert v['error']=='ZeroDivisionError';add('zero_bands_unvalidated',v)
c=Cache(m,num_bands=-2);v=cap(lambda:c.add_fingerprint(np.array([1,2,3,4]),1));assert v['error']=='ValueError';add('negative_bands_delayed_error_after_id_stored',dict(error=v,stored_ids=list(c.fingerprints)))
c=Cache(MinHasher(0,random_state=0),num_bands=2);c.add_doc('abcdefgh',1);c.add_doc('different',2);assert c.get_all_duplicates()=={(1,2)};v=cap(lambda:c.get_all_duplicates(min_jaccard=.5));assert v['error']=='ZeroDivisionError';add('zero_seeds_all_collide_then_jaccard_zero_division',v)
c=Cache(MinHasher(4,random_state=0),num_bands=2);c.add_fingerprint(np.array([1,2,3,4],dtype=np.uint64),1);c.add_fingerprint(np.array([1,2,3,4],dtype=np.uint64),2);assert not c.get_all_duplicates(min_jaccard=1);add('threshold1_excludes_identical_strict_policy',True)
assert not c.get_all_duplicates(min_jaccard=float('nan'));add('nan_threshold_silently_filters_everything',True)
v=cap(lambda:MinHasher(4,random_state=np.random.RandomState(0)));assert v['error']=='TypeError';add('documented_randomstate_instance_rejected',v)
c=Cache(m,num_bands=2);c.add_fingerprint(np.array([1],dtype=np.uint64),1);assert len(c.fingerprints[1])==1;add('fingerprint_length_not_checked_against_recipe',1)
# Independent correct unsigned minimum of original Murmur functions, per seed and shingle.
lib=ctypes.CDLL(str(W/'libmurmur.dylib'));lib.hash32.argtypes=lib.hash64.argtypes=[ctypes.c_void_p,ctypes.c_int,ctypes.c_uint32,ctypes.c_void_p]
def raw(text,seed,bits):
 buf=ctypes.create_string_buffer(text);o=(ctypes.c_uint64*2)() if bits==64 else ctypes.c_uint32();getattr(lib,'hash'+str(bits))(buf,len(text),seed,ctypes.byref(o));return int(o[0] if bits==64 else o.value)
seeds=np.array(range(32),dtype=np.uint32);cases=[]
for bits,fn in [(32,minhash_32),(64,minhash_64)]:
 mismatches=[];tested=0
 for i in range(32):
  text=bytes([65+i])+b'bcdefgh';ngram=8
  actual=list(map(int,fn(text,len(text),seeds,ngram)));expected=[raw(text,int(s),bits) for s in seeds];tested+=len(seeds)
  for j,(a,b) in enumerate(zip(actual,expected)):
   if a!=b:mismatches.append(dict(text_hex=text.hex(),seed=int(seeds[j]),actual=a,unsigned_minimum=b))
 assert mismatches
 cases.append(dict(bits=bits,comparisons=tested,mismatches=len(mismatches),first=mismatches[0]))
add('single_shingle_unsigned_murmur_minimum_oracle',cases)
# Correct long-input64minimum when at least one hash below clipped sentinel; all seeds/shingles recomputed.
text=b'The quick brown fox jumps over the lazy dog';k=5;actual=list(map(int,minhash_64(text,len(text),seeds,k)));expected=[min(raw(text[i:i+k],int(s),64) for i in range(len(text)-k+1)) for s in seeds];assert actual==expected;add('long64_complete_shingle_unsigned_oracle',dict(seeds=32,shingles=len(text)-k+1,all_equal=True))
for p in ['cache.py','minhash.py','cMinhash.pyx','MurmurHash3.cpp','MurmurHash3.h']:assert (B/'lsh'/p).read_bytes()==(S/'lsh'/p).read_bytes()
report=dict(scope='Unchanged fullcache/minhash classes andCython.pyx/Murmur implementation actuallyexecute throughCython3.3regenerated nativeextension onPython3.12/NumPy2.3.5. Buildcopysetup onlyUSE_CYTHONFalse->True andgeneratedcpp replaced. OriginalCython0.24.1cpp failslongintrepr.h; no oldbinaryparity/defaultbuild/platform/performance/productionadoption proof.',controls=out,source_pins={p:h(S/p) for p in ['lsh/cache.py','lsh/minhash.py','lsh/cMinhash.pyx','lsh/MurmurHash3.cpp','lsh/MurmurHash3.h','lsh/test/test_cache.py']},native_extension={str(p.relative_to(R)):h(p) for p in (B/'lsh').glob('*.so')},oracle_binary_sha256=h(W/'libmurmur.dylib'),oracle_wrapper_sha256=h(W/'murmur-wrapper.cpp'),producer_sha256=h(__file__),numpy_version=np.__version__)
p=D/f'lsh27-native-run{sys.argv[1]}-20261016.json';assert not p.exists();p.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(dict(controls=len(out),oracle=cases)))
