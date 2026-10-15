"""Exercise admitted native buffer layouts and rejected dtypes without unsafe pointer-length inputs."""
from pathlib import Path
import hashlib,json,sys
R=Path(__file__).resolve().parents[1];W=R/'.dedup-work/lsh27-20261016';B=W/'modern-build';sys.path.insert(0,str(B));import numpy as np
from lsh.cMinhash import minhash_64
from lsh.minhash import MinHasher
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();text=b'abcdefghijklmnopqrstuvwxyz';seeds=np.arange(8,dtype=np.uint32);expected=minhash_64(text,len(text),seeds,4);rows=[]
def obs(name,fn):
 try:
  value=fn();value=value.tolist() if isinstance(value,np.ndarray) else value;row=dict(control=name,value=value)
 except Exception as e:row=dict(control=name,error=type(e).__name__,message=str(e))
 rows.append(row);return row
for name,a,e in [('strided',seeds[::2],expected[::2]),('reversed',seeds[::-1],expected[::-1])]:
 v=minhash_64(text,len(text),a,4);assert np.array_equal(v,e);obs(name+'_seed_buffer_preserves_recipe',lambda:v)
a=seeds.copy();a.flags.writeable=False;v=obs('readonly_seed_buffer',lambda:minhash_64(text,len(text),a,4));assert v.get('value')==expected.tolist()
for name,a in [('big_endian',seeds.astype('>u4')),('uint64',seeds.astype(np.uint64)),('two_dimensional',seeds.reshape(2,4)),('none',None),('memoryview',memoryview(seeds))]:
 row=obs(name+'_seed_buffer_refused',lambda:minhash_64(text,len(text),a,4));assert row.get('error') in ['ValueError','TypeError']
z=MinHasher(8,char_ngram=0,random_state=0);assert np.array_equal(z.fingerprint('one document'),z.fingerprint('different document'));obs('zero_ngram_accepted_text_independent_fingerprint',lambda:z.fingerprint('one document'))
obs('empty_seed_buffer_returns_empty',lambda:minhash_64(text,len(text),np.array([],dtype=np.uint32),4));assert rows[-1]['value']==[]
report=dict(controls=rows,runs_scope='Actual unchanged .pyx via previouslybuiltCython3.3extension; safeowned buffers only, no malformedlength/negative-ngram pointer access, no oldCython0.24.1binaryparity/platform/performance claim.',producer_sha256=h(__file__),source_sha256=h(W/'source/lsh/cMinhash.pyx'),native_extensions={str(p.relative_to(R)):h(p) for p in (B/'lsh').glob('*.so')})
p=R/f'docs/research/dedup-500/lsh27-buffer-run{sys.argv[1]}-20261016.json';assert not p.exists();p.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(dict(controls=len(rows))))
