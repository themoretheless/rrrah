"""Whole unchanged upstream module, actual APFS and pinned dependencies; only owned synthetic files."""
from pathlib import Path
import importlib.util,sys,tempfile,json,os,contextlib,subprocess,hashlib
from unittest.mock import patch
R=Path(__file__).resolve().parents[3];W=R/'.dedup-work/clone111-20261015';sys.path.insert(0,str(W/'deps'));S=Path(__file__).with_name('clone111-full-source-20261015')/'clonefile-dedup.py';spec=importlib.util.spec_from_file_location('clone111',S);m=importlib.util.module_from_spec(spec);sys.modules[spec.name]=m;spec.loader.exec_module(m);assert m._clonefile_func is not None and m.HAS_XATTR
passed=[];cases=[]
def check(name,fn):cases.append((name,fn))
def pair(p):
 a,b=p/'a',p/'b';a.write_bytes(b'A'*4096);b.write_bytes(a.read_bytes());return a,b
def group(d,a,b):
 return {m.hash_file(a):[m.FileInfo(a,a.stat().st_size),m.FileInfo(b,b.stat().st_size)]}
def positive(p):
 a,b=pair(p);d=m.Deduplicator(p,workers=1,verbose=False);g=group(d,a,b);before=b.stat().st_ino;r=d.deduplicate(g,dry_run=False);assert r.files_replaced==1 and not r.errors and b.read_bytes()==a.read_bytes() and b.stat().st_ino!=before;b.write_bytes(b'B'*4096);assert a.read_bytes()==b'A'*4096
check('actual clone replacement and copy-on-write independence',positive)
def dry(p):
 a,b=pair(p);d=m.Deduplicator(p,verbose=False);before=b.stat().st_ino;r=d.deduplicate(group(d,a,b));assert r.files_replaced==0 and r.bytes_saved==4096 and b.stat().st_ino==before
check('dry run preserves inode and reports potential logical bytes',dry)
def target_mutation(p):
 a,b=pair(p);d=m.Deduplicator(p,verbose=False);g=group(d,a,b);b.write_bytes(b'NEW!'*1024);r=d.deduplicate(g,dry_run=False);assert r.files_replaced==1 and not r.errors and b.read_bytes()==b'A'*4096
check('changed duplicate overwritten without revalidation, verification succeeds',target_mutation)
def source_mutation(p):
 a,b=pair(p);d=m.Deduplicator(p,verbose=False);g=group(d,a,b);a.write_bytes(b'NEW!'*1024);r=d.deduplicate(g,dry_run=False);assert r.files_replaced==0 and len(r.errors)==1 and b.read_bytes()==a.read_bytes()
check('changed original overwrites duplicate before hash mismatch, no rollback',source_mutation)
def fixed_temp(p):
 a,b=pair(p);tmp=p/'b.clonetmp';tmp.write_bytes(b'owned unrelated marker');d=m.Deduplicator(p,verbose=False)
 with patch.object(m,'clonefile',side_effect=OSError('injected clone failure')):
  try:d._replace_with_clone(a,b);assert False
  except OSError:pass
 assert not tmp.exists() and b.read_bytes()==b'A'*4096
check('existing fixed temp marker deleted even when clone fails',fixed_temp)
def inherited_xattr(p):
 a,b=pair(p);m.xattr.setxattr(str(a),'org.rrrah.synthetic-source-only',b'source');m.xattr.setxattr(str(b),'org.rrrah.synthetic-dest-only',b'dest');d=m.Deduplicator(p,verbose=False);d._replace_with_clone(a,b);attrs=dict(m.xattr.xattr(str(b)));assert attrs['org.rrrah.synthetic-source-only']==b'source' and attrs['org.rrrah.synthetic-dest-only']==b'dest'
check('source-only xattr survives alongside destination attributes',inherited_xattr)
def xattr_failure(p):
 a,b=pair(p);m.xattr.setxattr(str(b),'org.rrrah.synthetic-dest-only',b'dest');d=m.Deduplicator(p,verbose=False)
 with patch.object(m.xattr,'setxattr',side_effect=OSError('injected attr failure')):r=d.deduplicate(group(d,a,b),dry_run=False)
 assert r.files_replaced==1 and not r.errors and 'org.rrrah.synthetic-dest-only' not in dict(m.xattr.xattr(str(b)))
check('xattr restoration failure silently reported as success',xattr_failure)
def parent_failure(p):
 a,b=pair(p);d=m.Deduplicator(p,verbose=False);before=b.stat().st_ino;real=m.os.utime
 def fail(path,*args,**kwargs):
  if Path(path)==p:raise OSError('injected parent timestamp failure')
  return real(path,*args,**kwargs)
 with patch.object(m.os,'utime',side_effect=fail):r=d.deduplicate(group(d,a,b),dry_run=False)
 assert len(r.errors)==1 and r.files_replaced==0 and b.stat().st_ino!=before
check('post-replace parent timestamp failure reports error after mutation',parent_failure)
def scan(p):
 a,b=pair(p);(p/'tiny').write_bytes(b'x');(p/'link').symlink_to(a);d=m.Deduplicator(p,verbose=False);assert d.scan()==2
check('scan minimum size and symlink exclusion',scan)
def missing(p):
 d=m.Deduplicator(p/'absent',verbose=False);r=d.run();assert r.files_scanned==0 and not r.errors
check('direct API missing root returns empty success',missing)
def full_pipeline(p):
 a,b=pair(p);(p/'c').write_bytes(b'C'*4096);d=m.Deduplicator(p,workers=1,verbose=False);r=d.run(dry_run=True);assert r.files_scanned==3 and r.duplicates_found==1 and r.bytes_saved==4096
check('actual process pool progressive hashing positive plus unequal negative',full_pipeline)
def stale(p):
 a,b=pair(p);d=m.Deduplicator(p,workers=1,verbose=False);d.scan();assert len(d.find_duplicates())==1;b.unlink();g=d.find_duplicates();assert len(g)==1 and any(x.path==b for xs in g.values() for x in xs)
check('second find without scan retains stale hashes after unreadable file',stale)
def cli(p):
 result=subprocess.run([sys.executable,str(S),str(p/'missing')],env=dict(os.environ,PYTHONPATH=str(W/'deps'),PYTHONDONTWRITEBYTECODE='1'),capture_output=True,text=True);assert result.returncode==1 and 'does not exist' in result.stderr
check('actual CLI missing root is rejected',cli)
if __name__=='__main__':
 for name,fn in cases:
  with tempfile.TemporaryDirectory(dir=W,prefix='control-') as p:fn(Path(p))
  passed.append(name);print('PASS',name,flush=True)
 print(json.dumps(dict(passed=len(passed),failed=0,scope='Whole upstream module, actual MacAPFS clonefile and hash/processpool dependencies; explicit fault injections identified. Tiny synthetic files only; no physical storage savings/largefiles/platform/performance qualification.')))
