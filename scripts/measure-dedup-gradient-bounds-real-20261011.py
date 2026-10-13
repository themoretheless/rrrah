"""Frozen alternating whole-matcher timing/parity on real decoded photo descriptors."""
import hashlib,json,os,re,shutil,subprocess,time,statistics
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research';T=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
OUT=D/'dedup-gradient-bounds-real-20261011.json';assert not OUT.exists()
MANIFEST=D/'dedup-gradient-bounds-real-snapshot-20261011.json';m=json.loads(MANIFEST.read_text());sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert all(sha(Path(m['snapshot'])/p)==h for p,h in m['file_hashes'].items())
assert all(sha(ROOT/p)==h for p,h in m['file_hashes'].items() if p.startswith('crates/rrrah-dedup/'))
build=D/'dedup-gradient-bounds-real-build-20261011.log';text=build.read_text();match=re.search(r'Executable .*\((target/debug/deps/[^)]+)\)',text);assert match,text[-1000:]
binary=T/'gradient-bounds-real-qualified-20261011';assert not binary.exists();shutil.copy2(ROOT/match[1],binary)
paths=[T/'original-resolution-negative-200201/200300.jpg',T/'original-resolution-strong-all/200302.jpg',T/'original-resolution-negative-200201/201400.jpg']
pins={str(p):sha(p) for p in [binary,MANIFEST,build,Path(__file__),*paths]}
report={'status':'running','pins':pins,'producer_pid':os.getpid(),'scope':__doc__};OUT.write_text(json.dumps(report,indent=2)+'\n')
env=os.environ.copy();env['RRRAH_GRADIENT_BENCH_FILES']=':'.join(map(str,paths));start=time.monotonic()
p=subprocess.run([str(binary),'gradient::matching_tests::real_photo_matcher_timing','--ignored','--nocapture'],env=env,text=True,capture_output=True)
report.update(returncode=p.returncode,elapsed_seconds=time.monotonic()-start,stdout=p.stdout,stderr=p.stderr)
try:
 assert p.returncode==0 and '1 passed; 0 failed' in p.stdout
 samples=[]
 for pair,n,k,radius,fast,count,seconds in re.findall(r'real_matcher pair=(\S+) features=(\d+):(\d+) radius=(\S+) bounded=(true|false) matches=(\d+) seconds=([0-9.]+)',p.stderr):
  samples.append(dict(pair=pair,left_features=int(n),right_features=int(k),radius=radius,bounded=fast=='true',matches=int(count),seconds=float(seconds)))
 assert len(samples)==24
 summary=[]
 for pair in ['0:0','0:1','0:2']:
  for radius in ['None','Some(64.0)']:
   rows=[s for s in samples if s['pair']==pair and s['radius']==radius];assert len(rows)==4 and [s['bounded'] for s in rows]==[False,True,True,False]
   assert len({s['matches'] for s in rows})==1
   medians={str(f):statistics.median(s['seconds'] for s in rows if s['bounded']==f) for f in [False,True]}
   summary.append(dict(pair=pair,radius=radius,median_seconds=medians,full_to_bounded_ratio=medians['False']/medians['True']))
 assert all(sha(p)==h for p,h in pins.items())
 report.update(status='complete_six_real_descriptor_controls_with_exact_parity',samples=samples,summary=summary,limits='Same-process alternating complete matcher, real features from three photos, self-positive plus crumpled and foreign pairs. Current test profile opt-level1 and shared-host load. No whole-library or Native6 collection speedup claim.')
except Exception as e:report.update(status='failed',error=str(e));raise
finally:OUT.write_text(json.dumps(report,indent=2)+'\n')
print(report['status'],flush=True)
