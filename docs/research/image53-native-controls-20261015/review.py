"""Original backend/IPC. Explicit scheduling, CPU count, writer and mutation controls."""
from pathlib import Path
import sys,os,json,hashlib,tempfile,subprocess,platform,io,contextlib,threading,multiprocessing as mp,gc
R=Path(__file__).resolve().parents[3];W=Path('/private/tmp/rrrah-image53-native-20261015');WH=Path('/private/tmp/rrrah-image53-wheels-20261015');wheels=sorted(WH.glob('*.whl'));assert len(wheels)==3
sys.path[:0]=[str(W)]+[str(p) for p in wheels]
from src import imagedups as m
from PIL import Image,ImageDraw
import PIL,numpy,imagehash,progressbar
assert imagehash.__version__=='4.3.2' and progressbar.__version__=='4.5.0'
OBS={}
def equal(a,b):assert a==b,(a,b)
def raises(kind,fn):
 try:fn()
 except kind:return
 raise AssertionError('expected '+kind.__name__)
def pic(path,tone=0):Image.new('RGB',(16,16),(tone,tone,tone)).save(path)
def config(d,**kw):
 out=dict(paths=[str(d)],recurse=False,quiet=False,delete=False,noprompt=True,minsize=None,maxsize=None);out.update(kw);return out
def full(cfg):
 original=m.mp.cpu_count;stdout=io.StringIO()
 try:
  m.mp.cpu_count=lambda:1
  with contextlib.redirect_stdout(stdout):m.dupes(cfg)
 finally:m.mp.cpu_count=original
 gc.collect();return stdout.getvalue()
def flat256(d):
 results={}
 for tone in range(256):
  p=d/f'{tone:03}.png';pic(p,tone);m.async_hash(str(p),results,threading.Lock())
 equal(set(results),{'0000000000000000'});equal(len(results['0000000000000000']),256)
 with Image.open(d/'000.png') as a,Image.open(d/'255.png') as b:assert a.tobytes()!=b.tobytes()
 OBS['flat256']=dict(distinct_gray_pixels=256,hashes=1)
def identical(d):
 a=d/'a.png';b=d/'b.png';pic(a);b.write_bytes(a.read_bytes());r={};lock=threading.Lock();m.async_hash(str(a),r,lock);m.async_hash(str(b),r,lock);equal(next(iter(r.values())),[str(a),str(b)])
def bad_image(d):
 p=d/'bad';p.write_bytes(b'not-an-image');r={};equal(m.async_hash(str(p),r,threading.Lock()),None);equal(r,{})
class FailWrite(dict):
 def __setitem__(self,k,v):raise OSError('owned injected publication failure')
def publish_failure(d):
 p=d/'a.png';pic(p);r=FailWrite();equal(m.async_hash(str(p),r,threading.Lock()),None);equal(dict(r),{})
class ScheduledProxy:
 def __init__(self,d,barrier,snapshots,commits):self.d=d;self.barrier=barrier;self.snapshots=snapshots;self.commits=commits
 def get(self,key,default):
  result=self.d.get(key,default);self.snapshots[os.getpid()]=len(result);self.barrier.wait(timeout=20);return result
 def __setitem__(self,key,value):self.d[key]=value;self.commits[os.getpid()]=len(value)
def lost_update(d):
 a=d/'a.png';b=d/'b.png';pic(a);pic(b);ctx=mp.get_context('spawn');runs=[]
 for repeat in range(3):
  with ctx.Manager() as manager:
   result=manager.dict();snapshots=manager.dict();commits=manager.dict();lock=manager.Lock();proxy=ScheduledProxy(result,ctx.Barrier(2),snapshots,commits);ps=[ctx.Process(target=m.async_hash,args=(str(p),proxy,lock)) for p in [a,b]]
   try:
    for p in ps:p.start()
    for p in ps:p.join(25)
    assert all(not p.is_alive() and p.exitcode==0 for p in ps);equal(sorted(snapshots.values()),[0,0]);equal(sorted(commits.values()),[1,1]);values=list(result.values());equal(len(values),1);equal(len(values[0]),1);assert values[0][0] in [str(a),str(b)];runs.append(dict(processes=2,read_snapshot_lengths=sorted(snapshots.values()),published_lengths=sorted(commits.values()),final_members=len(values[0])))
   finally:
    for p in ps:
     if p.is_alive():p.terminate();p.join(5)
 OBS['lost_update']=runs
def repeated_root_deletes_only_file(d):
 p=d/'only.png';pic(p);full(config(d,paths=[str(d),str(d)],delete=True));assert not p.exists();OBS['repeated_root']=dict(input_files=1,remaining_files=0)
def minmax_ignored(d):
 a=d/'a.png';b=d/'b.png';pic(a);b.write_bytes(a.read_bytes());assert 1<a.stat().st_size<10**9;text=full(config(d,minsize=10**9,maxsize=1));assert '[+]' in text and '[-]' in text and a.exists() and b.exists()
def different_pixels_deleted(d):
 a=d/'a.png';b=d/'b.png';pic(a,0);pic(b,255)
 with Image.open(a) as aa,Image.open(b) as bb:assert aa.tobytes()!=bb.tobytes()
 assert a.read_bytes()!=b.read_bytes();full(config(d,delete=True));equal(sum(p.exists() for p in [a,b]),1)
def recursion_mutates_input_paths(d):
 child=d/'child';child.mkdir();p=child/'only.png';pic(p);cfg=config(d,recurse=True);full(cfg);equal(len(cfg['paths']),2);text=full(cfg);equal(len(cfg['paths']),3);equal(cfg['paths'].count(str(child)),2);assert '[+]' in text and '[-]' in text and p.exists()
def stale_deletion(d):
 a=d/'a.png';b=d/'b.png';pic(a);pic(b);order=[d/n for n in os.listdir(d)];target=order[1];original=m.os.path.getsize;observed={}
 def changed(path):
  if str(path)==str(target) and not observed:
   observed['triggered']=True;im=Image.new('RGB',(16,16),'black');ImageDraw.Draw(im).rectangle((8,0,15,15),fill='white');im.save(target)
   with Image.open(target) as decoded:observed['new_hash']=str(imagehash.average_hash(decoded))
   assert observed['new_hash']!='0000000000000000'
  return original(path)
 try:m.os.path.getsize=changed;full(config(d,delete=True))
 finally:m.os.path.getsize=original
 assert observed['triggered'] and not target.exists() and order[0].exists();OBS['stale_deletion']=dict(new_distinct_hash=observed['new_hash'],changed_file_deleted=True)
def pattern(alpha=None):
 im=Image.new('RGB' if alpha is None else 'RGBA',(16,16),(0,0,0) if alpha is None else (0,0,0,alpha));ImageDraw.Draw(im).rectangle((8,0,15,15),fill=(255,255,255) if alpha is None else (255,255,255,alpha));return im
def animated_tail_ignored(d):
 paths=[]
 for i,tone in enumerate([0,255]):
  p=d/f'{i}.gif';pattern().save(p,save_all=True,append_images=[Image.new('RGB',(16,16),(tone,)*3)],duration=[50,50],loop=0,optimize=False);paths.append(p)
 assert paths[0].read_bytes()!=paths[1].read_bytes();frames=[]
 for p in paths:
  with Image.open(p) as im:equal(im.n_frames,2);im.seek(1);frames.append(im.convert('RGB').tobytes())
 assert frames[0]!=frames[1];r={}
 for p in paths:m.async_hash(str(p),r,threading.Lock())
 equal(len(r),1);equal(len(next(iter(r.values()))),2)
def alpha_ignored(d):
 paths=[]
 for i,alpha in enumerate([0,255]):p=d/f'{i}.png';pattern(alpha).save(p);paths.append(p)
 with Image.open(paths[0]) as a,Image.open(paths[1]) as b:assert a.tobytes()!=b.tobytes()
 r={}
 for p in paths:m.async_hash(str(p),r,threading.Lock())
 equal(len(r),1);equal(len(next(iter(r.values()))),2)
def cli(d):
 a=d/'a.png';b=d/'b.png';pic(a);b.write_bytes(a.read_bytes());code='import sys;from src import imagedups as m;m.mp.cpu_count=lambda:1;m.main(sys.argv[1:])';env=dict(os.environ,PYTHONDONTWRITEBYTECODE='1',PYTHONPATH=os.pathsep.join([str(W)]+[str(p) for p in wheels]));r=subprocess.run([sys.executable,'-c',code,'-p',str(d)],cwd=W,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE);equal(r.returncode,0);assert b'[+]' in r.stdout and b'[-]' in r.stdout and a.exists() and b.exists()
def literal_prompt(d):
 a=d/'a.png';b=d/'b.png';pic(a);b.write_bytes(a.read_bytes());old=sys.stdin
 try:sys.stdin=io.StringIO('y\n');text=full(config(d,delete=True,noprompt=False))
 finally:sys.stdin=old
 assert 'Delete %s? [y/n]' in text;equal(sum(p.exists() for p in [a,b]),1)
def missing_root(d):raises(FileNotFoundError,lambda:full(config(d/'missing')))
if __name__=='__main__':
 cases=[flat256,identical,bad_image,publish_failure,lost_update,repeated_root_deletes_only_file,minmax_ignored,different_pixels_deleted,recursion_mutates_input_paths,stale_deletion,animated_tail_ignored,alpha_ignored,cli,literal_prompt,missing_root];passed=[]
 for f in cases:
  with tempfile.TemporaryDirectory(prefix='rrrah-image53-owned-',dir='/private/tmp') as td:f(Path(td))
  passed.append(f.__name__);print('PASS',f.__name__,flush=True)
 print(json.dumps(dict(status='passed15_native_original_module_controls',cases=len(passed),case_names=passed,observations=OBS,python=platform.python_version(),Pillow=PIL.__version__,numpy=numpy.__version__,imagehash=imagehash.__version__,progressbar=progressbar.__version__,wheel_sha256={str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in wheels},source_sha256=hashlib.sha256((W/'src/imagedups.py').read_bytes()).hexdigest(),scope='RealImageHash/Pillow/NumPy/Progressbar and spawnProcesses/Manager/Lock, ownedFS/delete/CLI. Explicit read-barrier scheduler for3two-process lost-update schedules; CPUcount1resourceadapter forwholedupes; explicit writer/getsize/stdin controls. No corpus/platform/scale/performance/foreignproductionadoption proof.')))
