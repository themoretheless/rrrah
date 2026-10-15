"""Whole unchanged upstream CLI on owned synthetic fixtures only."""
from pathlib import Path
import hashlib,json,os,re,selectors,subprocess,time
R=Path(__file__).resolve().parents[1];W=R/'.dedup-work/ded23-20261015';exe=W/'target/debug/deduplicator'
assert exe.is_file()
env=dict(os.environ,TMPDIR=str(W),RAYON_NUM_THREADS='2')
root=W/('cli-fixtures-'+str(time.time_ns()));root.mkdir();results=[]
def fixture(name,contents):
 d=root/name;d.mkdir()
 for name,b in contents.items():(d/name).write_bytes(b)
 return d
def run(d,flags=(),data=None):
 p=subprocess.run([str(exe),str(d),*flags],input=data,text=True,capture_output=True,timeout=30,env=env)
 assert p.returncode==0,vars(p)
 return {'returncode':p.returncode,'stdout':p.stdout,'stderr':p.stderr}
same={'a.bin':b'same-content','b.bin':b'same-content'}
d=fixture('strict-permutation',{'a.bin':b'p'*16384+b'a'*4096+b'b'*4096+b'p'*16384,'b.bin':b'p'*16384+b'b'*4096+b'a'*4096+b'p'*16384,'c.bin':b'p'*16384+b'a'*4096+b'b'*4096+b'p'*16384})
assert (d/'a.bin').read_bytes()!=(d/'b.bin').read_bytes()
r=run(d,['--strict']);assert all(x in r['stdout'] for x in ['a.bin','b.bin','c.bin']);assert len(re.findall(r'\x1b\[33m[0-9a-f]+\x1b\[0m',r['stdout']))==1
results.append(dict(name='whole_strict_cli_groups_reordered_unequal_bytes',**r))
d=fixture('all-delete',same);r=run(d,['--strict','--interactive'],'0,1\ny\n');assert not list(d.iterdir());results.append(dict(name='interactive_all_indices_removes_all_copies',**r))
d=fixture('invalid-index',same);r=run(d,['--strict','--interactive'],'garbage\ny\n');assert len(list(d.iterdir()))==1 and r['stdout'].count('DELETED:')==1;results.append(dict(name='invalid_index_defaults_zero_and_deletes',**r))
d=fixture('hardlinks',{'a.bin':b'same-content'});os.link(d/'a.bin',d/'b.bin');assert os.stat(d/'a.bin').st_ino==os.stat(d/'b.bin').st_ino;r=run(d,['--strict','--interactive'],'0,1\ny\n');assert not list(d.iterdir());results.append(dict(name='whole_cli_accepts_hardlink_set_and_deletes_all_names',**r))
d=fixture('invalid-minimum',{'a.bin':b'','b.bin':b''});r=run(d,['--strict','--min-size','invalid']);assert 'a.bin' in r['stdout'] and 'b.bin' in r['stdout'];results.append(dict(name='invalid_minimum_silently_admits_zero_length',**r))
d=fixture('default-empty',{'a.bin':b'','b.bin':b''});r=run(d,['--strict']);assert 'No duplicates found' in r['stdout'] and 'a.bin' not in r['stdout'];results.append(dict(name='default_minimum_excludes_empty_files',**r))
d=fixture('changed-after-prompt',same)
p=subprocess.Popen([str(exe),str(d),'--strict','--interactive'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,env=env)
sel=selectors.DefaultSelector();sel.register(p.stdout,selectors.EVENT_READ);prefix=b'';deadline=time.monotonic()+30
try:
 while not prefix.endswith(b'\n> '):
  if time.monotonic()>deadline:raise TimeoutError('interactive prompt missing')
  if sel.select(.2):
   b=os.read(p.stdout.fileno(),1)
   if not b:raise RuntimeError('exited before prompt: '+repr(prefix))
   prefix+=b
 m=re.search(rb'^\s*0\s*\|\s*([ab]\.bin)\s*\|',prefix,re.M);assert m,prefix
 victim=d/m[1].decode();victim.write_bytes(b'owned-unique-content-after-hashing')
 out,err=p.communicate(b'0\ny\n',timeout=30);assert p.returncode==0
 assert not victim.exists();left=list(d.iterdir());assert len(left)==1 and left[0].read_bytes()==b'same-content'
 results.append({'name':'unique_content_written_after_hash_deleted_by_whole_cli','returncode':p.returncode,'stdout':(prefix+out).decode(),'stderr':err.decode(),'selected_victim':victim.name,'mutation_point':'after original first instruction prompt, before entering0andconfirmingy'})
finally:
 sel.close()
 if p.poll() is None:p.kill();p.wait()
print(json.dumps({'status':'passed','controls':len(results),'binary':str(exe),'binary_sha256':hashlib.sha256(exe.read_bytes()).hexdigest(),'fixture_root':str(root),'rows':results,'limits':['All deletions confined to newly authored synthetic fixtures','Actualwhole upstream CLI/Server/Interactive execution on MacARM, twoRayonthreads; no speed measurement','Prompt mutation explicitly scheduled, not ambient race or platform guarantee','Allcopy deletion reflects missing guard; original program accepts user choices, not productionlibrary operation']},indent=2))
