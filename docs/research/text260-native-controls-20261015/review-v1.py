"""Full unchanged module, real locked tqdm and multiprocessing; owned fixtures only."""
from pathlib import Path
import sys,os,json,hashlib,tempfile,subprocess,platform
R=Path(__file__).resolve().parents[3];W=Path('/private/tmp/rrrah-text260-native-20261015');WH=W/'tqdm-4.67.3-py3-none-any.whl';assert len(WH.read_bytes())==78374 and hashlib.sha256(WH.read_bytes()).hexdigest()=='ee1e4c0e59148062281c49d80b25b67771a127c85fc9676d3be5f243206826bf'
sys.path[:0]=[str(W),str(WH)]
from text_dedup import encoder as e
import tqdm
assert tqdm.__version__=='4.67.3'
def equal(a,b):assert a==b,(a,b)
def raises(kind,f):
 try:f()
 except kind:return
 raise AssertionError('expected '+kind.__name__)
def run_task(d,text='Alpha\nAlpha\nBeta\n',**kwargs):
 p=d/'input.txt';p.write_text(text,encoding='utf-8');args=dict(inputs=[str(p)],output=str(d/'output.txt'),shard_root=str(d/'shards'),chunksize=2,n_processes=1,prefix_length=2);args.update(kwargs);e.task_dedup(**args);return d/'output.txt',d/'shards'
def seeded(d,lines):
 root=d/'shards';enc=e.Encoder('sha1',e.Normalizer());e.save_shards(str(root),[enc(x) for x in lines],2);return root
def positive(d):
 out,shard=run_task(d);equal(set(out.read_text().splitlines()),{'Alpha','Beta'});assert not shard.exists();equal((d/'input.txt').read_text(),'Alpha\nAlpha\nBeta\n')
def unicode_collision(d):
 enc=e.Encoder('sha1',e.Normalizer());a=enc('Привет');b=enc('世界');equal(a[1],b[1]);root=d/'shards';e.save_shards(str(root),[a,b],2);out=d/'output';e.task_merge(str(out),str(root),2);equal(out.read_text().splitlines(),['Привет'])
def merge_append(d):
 root=seeded(d,['Alpha','Alpha']);out=d/'output';e.task_merge(str(out),str(root),2);e.task_merge(str(out),str(root),2);equal(out.read_text(),'Alpha\nAlpha\n')
def repeated_task(d):
 out,_=run_task(d,'Alpha\nAlpha\n',keep=True);run_task(d,'Alpha\nAlpha\n',keep=True);equal(out.read_text(),'Alpha\nAlpha\n')
def stale_shard(d):
 seeded(d,['Old']);out,_=run_task(d,'New\n',keep=True);equal(set(out.read_text().splitlines()),{'Old','New'})
def output_overlap(d):
 out,_=run_task(d,'Alpha\n',output=str(d/'shards/inside.txt'));assert not out.exists() and not (d/'shards').exists();assert (d/'input.txt').exists()
def oversized_block(d):
 root=seeded(d,['abcdefgh']);out=d/'output';e.task_merge(str(out),str(root),2,max_block_size=5);assert not out.exists() and not (d/'output.0').exists();equal((d/'output.1').read_bytes(),b'abcdefgh\n');assert (d/'output.1').stat().st_size>5
 def_unused=0

def no_matches(d):raises(ZeroDivisionError,lambda:e.task_dedup([str(d/'missing*.txt')],str(d/'output'),str(d/'shards'),2,1,prefix_length=2))
def excessive_prefix(d):
 raises(ZeroDivisionError,lambda:run_task(d,'Alpha\n',prefix_length=42,keep=True));assert list((d/'shards').rglob('*.shard'))
def space_filename(d):
 p=d/'gap long.txt';p.write_text('Alpha\n');raises(IndexError,lambda:e.encode_a_file(str(p),str(d/'shards'),2,1,e.Encoder('sha1',e.Normalizer()),2))
def invalid_worker(d):raises(AssertionError,lambda:run_task(d,'Alpha\n',n_processes=0))
def sort_rejected(d):raises(ValueError,lambda:run_task(d,'Alpha\n',sort=True,keep=False))
class ConstantDigest:
 def __init__(self,data):pass
 def hexdigest(self):return '0'*40
def callable_collision(d):
 enc=e.Encoder(ConstantDigest,e.Normalizer());a,b=enc('one'),enc('two');assert a[0]!=b[0] and a[1]==b[1];root=d/'shards';e.save_shards(str(root),[a,b],2);out=d/'output';e.task_merge(str(out),str(root),2);equal(out.read_text(),'one\n')
def lax_sizes(d):equal([e.humanized_to_number(x) for x in [-1,'2mbjunk','1.5kb'] ],[-1,2097152,1536])
def cli(d):
 p=d/'input';p.write_text('Alpha\nAlpha\nBeta\n');env=dict(os.environ,PYTHONPATH=os.pathsep.join([str(W),str(WH)]),PYTHONDONTWRITEBYTECODE='1');r=subprocess.run([sys.executable,'-m','text_dedup.cli','-i',str(p),'-s',str(d/'shards'),'-o',str(d/'output'),'-p','1','-c','2','-pr','2','-k'],env=env,cwd=W,stdout=subprocess.PIPE,stderr=subprocess.STDOUT);assert r.returncode==0,r.stdout.decode();equal(set((d/'output').read_text().splitlines()),{'Alpha','Beta'});assert (d/'shards').exists()
def directory_child(d):
 root=d/'input';(root/'child').mkdir(parents=True);raises(IsADirectoryError,lambda:e.task_dedup(str(root),str(d/'out'),str(d/'shards'),2,1,prefix_length=2))
def bad_utf8(d):
 p=d/'bad';p.write_bytes(b'\xff\xfe\n');raises(UnicodeDecodeError,lambda:e.task_dedup([str(p)],str(d/'out'),str(d/'shards'),2,1,prefix_length=2))
if __name__=='__main__':
 cases=[positive,unicode_collision,merge_append,repeated_task,stale_shard,output_overlap,oversized_block,no_matches,excessive_prefix,space_filename,invalid_worker,sort_rejected,callable_collision,lax_sizes,cli,directory_child,bad_utf8];passed=[]
 for f in cases:
  with tempfile.TemporaryDirectory(prefix='rrrah-text260-owned-',dir='/private/tmp') as tmp:f(Path(tmp))
  passed.append(f.__name__);print('PASS',f.__name__,flush=True)
 sources={str(p.relative_to(W)):hashlib.sha256(p.read_bytes()).hexdigest() for p in (W/'text_dedup').rglob('*.py')};print(json.dumps(dict(status='passed17_native_original_module_controls',cases=len(passed),case_names=passed,python=platform.python_version(),tqdm=tqdm.__version__,wheel=str(WH),wheel_sha256=hashlib.sha256(WH.read_bytes()).hexdigest(),source_sha256=sources,scope='Real Python3.12/locked tqdm4.67.3/Pool1/wc/owned filesystem/CLI subprocess; constant callablehash via supported API is explicit collision-sensitivity control, not real SHA1collision. No foreigncode adoption/performance/scale/platform claim.')))
