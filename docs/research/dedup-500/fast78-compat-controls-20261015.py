"""Explicit Python2-to3 syntax translation + byte-string/file IO compatibility; NOT native Python2."""
from pathlib import Path
import json,io,os,tempfile,hashlib,contextlib,builtins,types
from unittest.mock import patch
from lib2to3.refactor import RefactoringTool,get_fixers_from_package
R=Path(__file__).resolve().parents[3];D=Path(__file__).parent;W=R/'.dedup-work/fast78-20261015';W.mkdir(exist_ok=True);(W/'.gitignore').write_text('*\n');source=next(x['content'] for x in json.loads((D/'fast78-full-source-20261015.json').read_text())['blobs'] if x['row']['path']=='fastdupes.py');translated=str(RefactoringTool(get_fixers_from_package('lib2to3.fixes')).refactor_string(source,'upstream-py2'))
class Py2Bytes(bytes):
 def __eq__(self,other):return len(self)==0 if other=='' else bytes.__eq__(self,other)
 __hash__=bytes.__hash__
class Handle:
 def __init__(self,p,mode='rb'):self.f=builtins.open(p,mode)
 def read(self,n):return Py2Bytes(self.f.read(n))
 def close(self):self.f.close()
 def __del__(self):self.f.close()
def load(optimized=False):
 ns=dict(__name__='fast78_compat',file=Handle,open=Handle);exec(compile(translated,'upstream-syntax-translated','exec',optimize=2 if optimized else 0),ns);ns['out']=types.SimpleNamespace(write=lambda *a,**k:None);return types.SimpleNamespace(**ns),ns
m,ns=load();passed=[]
def check(name,fn):
 with tempfile.TemporaryDirectory(dir=W) as p,contextlib.redirect_stdout(io.StringIO()):fn(Path(p))
 passed.append(name);print('PASS',name,flush=True)
def pair(p):
 a,b=p/'a',p/'b';a.write_bytes(b'H'*16384+b'A'*100);b.write_bytes(b'H'*16384+b'B'*100);return a,b
def default_false(p):
 a,b=pair(p);g=m.find_dupes([str(p)]);assert len(g)==1 and set(next(iter(g.values())))=={str(a),str(b)} and a.read_bytes()!=b.read_bytes()
check('default final hash is same16KiB prefix, unequal tails falsely grouped',default_false)
def exact_negative(p):
 pair(p);assert m.find_dupes([str(p)],exact=True)=={}
check('exact mode rejects unequal tails after same header',exact_negative)
def exact_positive(p):
 a,b=pair(p);b.write_bytes(a.read_bytes());assert len(m.find_dupes([str(p)],exact=True))==1
check('exact mode identical positive',exact_positive)
def repeated(p):
 a,b=pair(p);b.unlink();assert m.find_dupes([str(p),str(p)])=={}
check('same path repeated does not self duplicate',repeated)
def hardlinks(p):
 a,b=pair(p);b.unlink();os.link(a,b);assert len(m.find_dupes([str(p)],exact=True))==1 and a.stat().st_ino==b.stat().st_ino
check('hardlink aliases counted as duplicate files',hardlinks)
def ignores(p):
 for name in ['skip1','skip2']:(p/name).mkdir();(p/name/'owned').write_bytes(b'x')
 paths=m.getPaths([str(p)],ignores=[str(p/'skip*')]);assert len(paths)==1 and 'skip' in paths[0]
check('mutating ignored directory list skips adjacent exclusion',ignores)
def explicit_link(p):
 a,b=pair(p);b.unlink();b.symlink_to(a);assert m.getPaths([str(b)])==[str(a)]
check('explicit symlink root realpath becomes admitted target',explicit_link)
def invalid_ui(p):
 with patch.dict(ns['pruneUI'].__globals__,input=lambda _: '0'):result=m.pruneUI(['a','b'])
 assert result==['a','b']
check('invalid numeric keeper0 proposes deleting every item',invalid_ui)
def assertion_guard(p):
 a,b=pair(p)
 with patch.dict(ns['pruneUI'].__globals__,input=lambda _: '0'):
  try:m.delete_dupes({'g':[str(a),str(b)]});assert False
  except AssertionError:pass
 assert a.exists() and b.exists()
check('normal assertion blocks all-delete invalid keeper',assertion_guard)
def optimized_guard(p):
 a,b=pair(p);o,on=load(True)
 with patch.dict(on['pruneUI'].__globals__,input=lambda _: '0'):o.delete_dupes({'g':[str(a),str(b)]})
 assert not a.exists() and not b.exists()
check('optimized translation removes safety assertion and deletes both owned files',optimized_guard)
def stale_delete(p):
 a,b=pair(p);b.write_bytes(a.read_bytes());groups=m.find_dupes([str(p)]);b.write_bytes(b'N'*len(b.read_bytes()));m.delete_dupes(groups,prefer_list=[str(a)],interactive=False);assert a.exists() and not b.exists()
check('changed target after discovery deleted without revalidation',stale_delete)
def default_delete(p):
 a,b=pair(p);groups=m.find_dupes([str(p)]);m.delete_dupes(groups,prefer_list=[str(a)],interactive=False);assert a.exists() and not b.exists()
check('prefix false positive can delete distinct tail file',default_delete)
def fullhash(p):
 a,b=pair(p);assert m.hashFile(str(a),limit=0)==hashlib.sha1(a.read_bytes()).digest();assert m.hashFile(str(a),limit=16384)==hashlib.sha1(a.read_bytes()[:16384]).digest()
check('explicit limit0 hashes full content,16KiB hashes prefix',fullhash)
def zerobuffer(p):
 a,b=pair(p);assert m.hashFile(str(a),limit=0,chunk_size=0)==hashlib.sha1(b'').digest()
check('zero chunk hashes empty payload despite nonempty input',zerobuffer)
def short_read(p):
 class Short:
  def read(self,n):return Py2Bytes(b'A')
 assert m.hashFile(Short(),limit=4,chunk_size=4)==hashlib.sha1(b'A').digest()
check('short read counts requested bytes and stops before advertised limit',short_read)
def noinput(p):assert m.find_dupes([str(p/'missing')])=={}
check('missing directory silently empty result',noinput)
print(json.dumps(dict(passed=len(passed),failed=0,scope='Whole module lib2to3 syntax translation, explicit Py2 byte/file EOF adapter. Native Python2, installed console wrapper and package/documentation build not executed. Optimized assertion removal control explicitly compile(optimize=2); only owned tiny files. No library performance/coverage qualification.')))
