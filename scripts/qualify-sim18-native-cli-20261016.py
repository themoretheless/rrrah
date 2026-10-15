"""Actual unchanged C++ CLI against owned binary fixtures, independent pair oracle."""
from pathlib import Path
import hashlib,itertools,json,resource,struct,subprocess,tempfile
R=Path(__file__).resolve().parents[1];W=R/'.dedup-work/sim18-20261016';exe=W/'core-cli';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();obs=[]
def no_core():resource.setrlimit(resource.RLIMIT_CORE,(0,0))
def run(data,blocks=4,radius=1,extra=None):
 args=[str(exe),'--blocks',str(blocks),'--distance',str(radius),'--input','-','--output','-'] if extra is None else [str(exe),*extra]
 return subprocess.run(args,input=data,stdout=subprocess.PIPE,stderr=subprocess.PIPE,preexec_fn=no_core,timeout=10)
def packed(v):return b''.join(struct.pack('=Q',x) for x in v)
def decoded(b):assert len(b)%16==0;return set(struct.iter_unpack('=QQ',b))
def record(name,**d):obs.append({'control':name,**d})
values=[0,1,3,7,2**63,2**64-1];oracle={(a,b) for a,b in itertools.combinations(sorted(values),2) if (a^b).bit_count()<=1};p=run(packed(values));assert p.returncode==0 and decoded(p.stdout)==oracle;record('actual_native_stdin_stdout_pairs',pairs=len(oracle),native_endian=True)
p=run(packed([17,17]));assert p.returncode==0 and p.stdout==b'';record('identical_hash_occurrences_coalesced',input_occurrences=2,pairs=0)
p=run(b'');assert p.returncode==0 and p.stdout==b'';record('empty_binary_input_succeeds',returncode=0)
for suffix in range(1,8):
 p=run(packed([0,1])+b'\xff'*suffix);assert p.returncode==0 and decoded(p.stdout)=={(0,1)};record('partial_trailing_record_silently_ignored',trailing_bytes=suffix,returncode=0,pairs=1)
p=run(b'\xff'*7);assert p.returncode==0 and p.stdout==b'';record('only_partial_record_silently_empty_success',bytes=7,returncode=0)
p=run(packed([0,1]),radius=0);assert p.returncode==3;record('cli_rejects_zero_distance',returncode=3,scope='CLI policy; library supports radius0')
p=run(packed([0,1]),blocks=65);assert p.returncode==-6 and b'invalid_argument' in p.stderr;record('excessive_blocks_abort_cli',returncode=-6,stderr=p.stderr.decode())
with tempfile.TemporaryDirectory(prefix='sim18-cli-owned-') as tmp:
 missing=Path(tmp)/'missing';p=run(b'',extra=['--blocks','4','--distance','1','--input',str(missing),'--output','-']);assert p.returncode==7;record('missing_input_returns_error',returncode=7)
 p=run(packed([0,1]),extra=['--blocks','4','--distance','1','--input','-','--output',tmp]);assert p.returncode==8;record('unopenable_output_returns_error',returncode=8)
print(json.dumps({'status':'passed_actual_native_cli_controls','observations':obs,'binary':str(exe),'binary_sha256':h(exe),'source_pins':{str(p.relative_to(W/'source')):h(p) for p in (W/'source/simhash/simhash-cpp').rglob('*') if p.is_file()},'scope':'Whole unchanged upstream C++CLI and2coretranslationunits compilednativeMacARM; ownedstdin/tempfixtures, hostnativeendian16bytepair records. No originalgtest/oldPythonfallback/platform/IO-faultmatrix/RSS/performance qualification.'},sort_keys=True))
