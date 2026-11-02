"""macOS native-test libc call count and per-child kernel RSS; not timing evidence."""
import hashlib,json,os,re,signal,subprocess,sys,time
from pathlib import Path
out=Path(__file__).resolve().parent
assert sys.platform=='darwin'
name=os.environ.get('RRRAH_PROBE_NAME','dicom')
source_log={'dicom':'dicom-pairs-2.log','mng':'mng-tests.log','webp':'animation-tests.log'}[name]
test={'dicom':'dicom::rr36_qualification::full_decode_tag_set_pairs','mng':'mng::tests::frame_limit_preallocation_spends_heap_before_invalid_header_rejection','webp':'webp_animation::tests::frame_limit_preallocation_spends_heap_before_invalid_header_rejection'}[name]
text=(out/source_log).read_text();binary=Path(re.search(r'Running unittests .*?\(([^)]+)\)',text)[1]).resolve()
result={'binary':str(binary),'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'scope':'process-wide intercepted libc allocation calls, per-child Darwin kernel peak RSS bytes; common fixture/test overhead included; instrumented runs excluded from performance timing','runs':[]}
for pair in range(10):
 for variant in (['legacy','candidate']if pair%2==0 else['candidate','legacy']):
  env=os.environ.copy();env.update(DYLD_INSERT_LIBRARIES='/tmp/rrrah-31-60-allocation-calls.dylib');env['RRRAH_DICOM_TAG_VARIANT' if name=='dicom' else 'RRRAH_FRAME_ALLOCATION_VARIANT']=variant
  log=out/f'{name}-allocation-{pair}-{variant}.log'
  with log.open('w')as stream:
   p=subprocess.Popen([str(binary),test,'--exact']+(['--ignored']if name=='dicom'else[])+['--show-output','--test-threads=1'],stdout=stream,stderr=subprocess.STDOUT,env=env,start_new_session=True)
   deadline=time.monotonic()+30
   while True:
    pid,status,usage=os.wait4(p.pid,os.WNOHANG)
    if pid:break
    if time.monotonic()>deadline:
     os.killpg(p.pid,signal.SIGKILL);os.wait4(p.pid,0);p.returncode=124;raise TimeoutError(str(log))
    time.sleep(0.01)
   p.returncode=os.waitstatus_to_exitcode(status)
  data=log.read_text();assert p.returncode==0,data;assert '1 passed; 0 failed; 0 ignored' in data
  counts=re.search(r'ALLOCATION_CALLS malloc=(\d+) calloc=(\d+) realloc=(\d+) aligned=(\d+)',data);assert counts,data
  result['runs'].append({'pair':pair,'variant':variant,'malloc':int(counts[1]),'calloc':int(counts[2]),'realloc':int(counts[3]),'aligned':int(counts[4]),'peak_rss_bytes':usage.ru_maxrss,'log':log.name})
  (out/f'{name}-allocation-results.json').write_text(json.dumps(result,indent=2)+'\n')
