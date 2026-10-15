"""Actual rebuilt unchanged upstream Python/C++ functions, independent Hamming/majority oracles."""
from pathlib import Path
import hashlib,itertools,json,os,random,resource,subprocess,sys,tempfile
R=Path(__file__).resolve().parents[1];W=R/'.dedup-work/sim18-20261016';S=W/'source';B=W/'build-source';sys.path.insert(0,str(B));import simhash
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();rng=random.Random(18);obs=[]
def record(name,**d):obs.append({'control':name,**d})
def pairs(values,radius):return {(a,b) for a,b in itertools.combinations(sorted(set(values)),2) if (a^b).bit_count()<=radius}
queries=0;matched=0
families=[list(range(256)),[rng.getrandbits(64) for _ in range(128)]];families[1]+=[v^(1<<i) for v in families[1][:12] for i in range(6)]
for values in families:
 for radius in [0,1,2,3]:
  expected=pairs(values,radius)
  for blocks in sorted({radius+1,radius+2,8}):
   before=list(values);actual=set(simhash.find_all(values,blocks,radius));assert actual==expected and values==before;queries+=1;matched+=len(actual)
record('native_complete_pairs_match_independent_hamming_oracle',queries=queries,matched_pairs_across_queries=matched,families=['all256low8bit','128seeded64bit+72nearneighbors'])
fixture=R/'crates/rrrah-dedup/tests/fixtures/near41-phash64.txt';lines=[x for x in fixture.read_text().splitlines() if x and not x.startswith('#')];print('FIXTURE_FIRST',lines[0],file=sys.stderr)
# The retained fixture stores row id then64bit hex fingerprint.
values=[int(line.split()[-1],16) for line in lines];assert len(values)==540;expected=pairs(values,8)
for blocks in [9,10]:assert set(simhash.find_all(values,blocks,8))==expected
record('public540image_hash_pairs_complete_for_distinct_hashes',rows=540,unique_hashes=len(set(values)),radius=8,blocks=[9,10],distinct_hash_pairs=len(expected),fixture_sha256=h(fixture),visual_quality_ground_truth=False)
for _ in range(1024):
 values=[rng.getrandbits(64) for _ in range(rng.randrange(25))];expected=sum(1<<bit for bit in range(64) if sum((v>>bit)&1 for v in values)*2>len(values));assert simhash.compute(values)==expected
record('majority_simhash_matches_independent_bit_counts',vectors=1024)
assert simhash.compute([0,2**64-1])==0;record('balanced_bit_votes_choose_zero',result=0)
assert simhash.find_all([17,17],4,3)==[];record('hash_set_discards_equal_document_fingerprints',input_occurrences=2,emitted_pairs=0,scope='DeclaredhashsetAPI; not a document-identity preserving index')
for name,call in [('negative_hash',lambda:simhash.compute([-1])),('too_large_hash',lambda:simhash.compute([2**64])),('negative_radius',lambda:simhash.find_all([0],4,-1))]:
 try:call()
 except OverflowError:record(name,exception='OverflowError')
 else:raise AssertionError(name)
assert list(simhash.shingle((x for x in range(5)),2))==[[0,1],[1,2],[2,3],[3,4]];record('streaming_shingles_keep_full_overlapping_windows',windows=4)
def no_core():resource.setrlimit(resource.RLIMIT_CORE,(0,0))
for blocks,bits in [(0,0),(4,4),(65,3)]:
 code='import sys;sys.path.insert(0,'+repr(str(B))+');import simhash;simhash.find_all([0,1],'+str(blocks)+','+str(bits)+')';p=subprocess.run([sys.executable,'-c',code],stdout=subprocess.PIPE,stderr=subprocess.PIPE,preexec_fn=no_core,timeout=10);assert p.returncode==-6 and b'invalid_argument' in p.stderr;record('invalid_parameters_abort_native_python_process',blocks=blocks,bits=bits,returncode=p.returncode,stderr=p.stderr.decode(errors='replace'))
with tempfile.TemporaryDirectory(prefix='sim18-owned-') as tmp:
 p=Path(tmp)/'hashes.txt';p.write_text('0\n1\n');env=dict(os.environ,PYTHONPATH=str(B));out=subprocess.run([sys.executable,str(B/'bench.py'),'--hashes',str(p)],env=env,cwd=tmp,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=10);assert out.returncode==1 and b'has no attribute' in out.stderr and b'split' in out.stderr;record('original_benchmark_file_input_raises',returncode=1,exception='AttributeError')
so=next((B/'simhash').glob('*.so'));pins={str(p.relative_to(S)):h(p) for p in S.rglob('*') if p.is_file()};assert (B/'test/test.py').read_bytes()==(S/'test/test.py').read_bytes();assert (B/'simhash/simhash.pyx').read_bytes()==(S/'simhash/simhash.pyx').read_bytes();print(json.dumps({'status':'passed_actual_native_controls','observations':obs,'source_pins':pins,'binary':str(so),'binary_sha256':h(so),'generated_cpp_sha256':h(B/'simhash/simhash.cpp'),'python':sys.version,'scope':'Actual rebuilt upstream Python package and pinned C++core, whole unchanged .pyx/.pxd and14originaltestmethods; modernCython3.3 directclang build withCYTHON_USE_CPP_STD_MOVE=0 afteroriginalsetup failures. No original2016Python/Cython environment/fallbackgeneratedC++/gtest/fullsourcegeneratedscaffold/platform/physicalIO/RSS/performance/visualquality proof.'},sort_keys=True))
