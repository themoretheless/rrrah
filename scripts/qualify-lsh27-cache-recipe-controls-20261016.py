"""Execute actual native MinHasher cache-recipe and returned-array ownership controls."""
from pathlib import Path
import gc,hashlib,json,sys
R=Path(__file__).resolve().parents[1];W=R/'.dedup-work/lsh27-20261016';B=W/'modern-build';sys.path.insert(0,str(B));import numpy as np
from lsh.minhash import MinHasher
from lsh.cache import Cache
from lsh.cMinhash import minhash_64
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();rows=[];text='abcdefghijklmnopqrstuvwxyz0123456789'
def add(name,**value):rows.append(dict(control=name,**value))
m=MinHasher(8,char_ngram=4,random_state=0);old=m.fingerprint(text);m.char_ngram=8;stale=m.fingerprint(text);fresh=MinHasher(m._seeds,char_ngram=8).fingerprint(text);assert stale is old and not np.array_equal(stale,fresh);add('changed_public_ngram_reuses_stale_cached_recipe',stale_same_object=True,different_from_fresh_recipe=True)
m=MinHasher(8,char_ngram=4,random_state=0,hashbytes=8);old=m.fingerprint(text);m.hashbytes=4;stale=m.fingerprint(text);fresh=MinHasher(m._seeds,char_ngram=4,hashbytes=4).fingerprint(text);assert stale is old and stale.dtype==np.uint64 and fresh.dtype==np.uint32;add('changed_public_hashbytes_returns_old_dtype',cached_dtype=str(stale.dtype),fresh_dtype=str(fresh.dtype))
# The shared method-level lru_cache is invalidated across unrelated hasher instances.
a=MinHasher(8,char_ngram=4,random_state=0);b=MinHasher(8,char_ngram=4,random_state=1);old=b.fingerprint(text);Cache(a,num_bands=2).clear();fresh=b.fingerprint(text);assert fresh is not old and np.array_equal(fresh,old);add('clear_one_cache_invalidates_other_hasher_cache',same_values=True,new_object=True)
seeds=np.arange(8,dtype=np.uint32);original=seeds.copy();data=text.encode();x=minhash_64(data,len(data),seeds,4);y=minhash_64(data,len(data),seeds,4);assert np.array_equal(seeds,original) and np.array_equal(x,y) and not np.shares_memory(x,y) and not np.shares_memory(x,seeds);add('direct_native_outputs_independent_input_unmodified',arrays_disjoint=True,seeds_unchanged=True)
assert x.flags.owndata and y.flags.owndata;add('direct_native_returned_arrays_own_storage',owndata=True)
expected=x.copy();del seeds,data,y;gc.collect();assert np.array_equal(x,expected);add('output_survives_input_release_and_gc',same_values=True)
report=dict(controls=rows,scope='WholeunchangedPython cache/minhash classes and previouslybuiltCython3.3regeneratedunchanged.pyx nativeextension, syntheticownedbuffers only. Publicrecipe mutation stale-cache and crossinstancecache-clear behavior; no oldCython0.24.1binaryparity/generalGC/RSS/platform/performance/productionadoption proof.',source_pins={p:h(W/'source'/p) for p in ['lsh/cache.py','lsh/minhash.py','lsh/cMinhash.pyx']},producer_sha256=h(__file__),native_extensions={str(p.relative_to(R)):h(p) for p in (B/'lsh').glob('*.so')})
p=R/f'docs/research/dedup-500/lsh27-cache-recipe-run{sys.argv[1]}-20261016.json';assert not p.exists();p.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(dict(controls=len(rows))))
