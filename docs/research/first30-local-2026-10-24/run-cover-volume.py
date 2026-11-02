"""Disposable APFS remount experiment. Remount-cold VFS is not physical cold storage."""
import json,os,subprocess,tempfile,time,hashlib
from pathlib import Path
out=Path(__file__).resolve().parent
root=Path.cwd()
result={'scope':'local disposable APFS; remount-cold VFS, not physical disk cache or network FS','pairs':[],'errors':[]}
def run(args):
 return subprocess.run(args,capture_output=True,text=True,timeout=40,check=True)
with tempfile.TemporaryDirectory(prefix='rrrah-cover-volume-') as directory:
 temp=Path(directory);image=temp/'cover.dmg';mount=temp/'mounted';mount.mkdir();attached=False
 def attach():
  global attached
  r=run(['hdiutil','attach',str(image),'-nobrowse','-mountpoint',str(mount)]);attached=True;return r.stdout
 def detach():
  global attached
  r=run(['hdiutil','detach',str(mount)]);attached=False;return r.stdout
 try:
  result['create']=run(['hdiutil','create','-size','128m','-fs','APFS','-volname','RrrahCoverProbe',str(image)]).stdout
  result['attach']=attach();fixtures=mount/'fixtures';fixtures.mkdir();(mount/'.metadata_never_index').touch()
  for i in range(10000): (fixtures/f'{i:05}.png' if i%10==0 else fixtures/f'{i:05}.junk').touch()
  (fixtures/'000-directory.png').mkdir();(fixtures/'000-link.png').symlink_to(fixtures/'00000.png')
  detach()
  # Freeze the exact already-built native test executable before measuring.
  manifest=json.loads((out/os.environ.get('RRRAH_COVER_MANIFEST','cover-binary.json')).read_text());binary=Path(manifest['path']);assert hashlib.sha256(binary.read_bytes()).hexdigest()==manifest['sha256']
  def probe(mode,trace=False):
   env=os.environ.copy();env.update(RRRAH_COVER_PROBE_ROOT=str(fixtures),RRRAH_COVER_PROBE_MODE=mode)
   if trace:env['DYLD_INSERT_LIBRARIES']=os.environ['RRRAH_COVER_TRACE_LIBRARY']
   proc=subprocess.Popen([str(binary),'gallery::tests::cover_directory_probe','--exact','--ignored','--show-output','--test-threads=1'],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
   try:stdout,stderr=proc.communicate(timeout=20)
   except subprocess.TimeoutExpired:
    sample=out/f'cover-delay-{proc.pid}.sample.txt'
    subprocess.run(['sample',str(proc.pid),'1','-file',str(sample)],capture_output=True,timeout=8)
    try:stdout,stderr=proc.communicate(timeout=35)
    except subprocess.TimeoutExpired:
     proc.kill();stdout,stderr=proc.communicate(timeout=5);result.setdefault('timed_out_processes',[]).append({'mode':mode,'stdout':stdout,'stderr':stderr,'sample':str(sample)});raise RuntimeError('cover timed out after sampling')
   r=subprocess.CompletedProcess(proc.args,proc.returncode,stdout,stderr)
   assert r.returncode==0,r.stdout+r.stderr
   assert '1 passed; 0 failed; 0 ignored' in r.stdout
   return r.stdout+r.stderr
  for pair in range(0 if os.environ.get('RRRAH_COVER_COUNTS_ONLY') else 10):
   for mode in (['baseline','candidate'] if pair%2==0 else ['candidate','baseline']):
    attach();cold=probe(mode);warm=probe(mode);detach();result['pairs'].append({'pair':pair,'mode':mode,'remount_cold':cold,'warm':warm})
  attach();result['instrumented_metadata_calls']={mode:probe(mode,True) for mode in ['baseline','candidate']};detach()
 except Exception as e:
  result['errors'].append(repr(e))
 finally:
  if attached:
   try:result['detach']=detach()
   except Exception as e:result['errors'].append('detach: '+repr(e))
  (out/os.environ.get('RRRAH_COVER_OUTPUT','cover-volume-results.json')).write_text(json.dumps(result,indent=2)+'\n')
if result['errors']:raise SystemExit(str(result['errors']))
