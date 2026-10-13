"""Alternating same-process four-lane baseline/current matcher on real photo features."""
import hashlib,json,os,re,statistics,subprocess,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research'
BASE=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
OUT=D/'dedup-gradient-mutual-real-20261011.json'
MANIFEST=D/'dedup-gradient-mutual-snapshot-20261011.json'
GATE=D/'dedup-gradient-mutual-resumed-generic-20261011.json'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 assert not OUT.exists();gate=json.loads(GATE.read_text())
 assert gate['status']=='complete_bounded_frozen_gradient_mutual_generic_execution' and gate['tests']==205
 manifest=json.loads(MANIFEST.read_text());snapshot=Path(manifest['snapshot'])
 assert all(sha(snapshot/p)==h for p,h in manifest['file_hashes'].items())
 result=next(r for r in gate['results'] if r['suite']=='rrrah_dedup');binary=Path(result['binary']);assert sha(binary)==result['binary_sha256']
 paths=[BASE/'original-resolution-negative-200201/200100.jpg',BASE/'original-resolution-strong-all/200001.jpg',BASE/'original-resolution-strong-all/200302.jpg']
 pins={str(p):sha(p) for p in [binary,GATE,MANIFEST,Path(__file__),*paths]}
 report={'status':'running','producer_pid':os.getpid(),'pins':pins,'scope':__doc__}
 def save():
  temp=OUT.with_suffix('.json.tmp');temp.write_text(json.dumps(report,indent=2)+'\n');temp.replace(OUT)
 save();environment=os.environ.copy();environment['RRRAH_GRADIENT_BENCH_FILES']=':'.join(map(str,paths))
 try:
  process=subprocess.Popen([str(binary),'gradient::mutual_tests::real_photo_mutual_competitor_timing','--ignored','--nocapture'],env=environment,text=True,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
  report['process_pid']=process.pid;save();start=time.monotonic();stdout,stderr=process.communicate()
  report.update(returncode=process.returncode,elapsed_seconds=time.monotonic()-start,stdout=stdout,stderr=stderr)
  assert process.returncode==0 and '1 passed; 0 failed' in stdout
  samples=[]
  for pair,n,k,radius,fast,count,seconds in re.findall(r'real_matcher pair=(\S+) features=(\d+):(\d+) radius=(\S+) mutual=(true|false) matches=(\d+) seconds=([0-9.]+)',stderr):
   samples.append({'pair':pair,'left_features':int(n),'right_features':int(k),'radius':radius,'mutual':fast=='true','matches':int(count),'seconds':float(seconds)})
  assert len(samples)==24;summaries=[]
  for pair in ['0:0','0:1','0:2']:
   for radius in ['None','Some(64.0)']:
    rows=[s for s in samples if s['pair']==pair and s['radius']==radius];assert len(rows)==4 and [s['mutual'] for s in rows]==[False,True,True,False]
    assert len({s['matches'] for s in rows})==1
    before=statistics.median(s['seconds'] for s in rows if not s['mutual']);after=statistics.median(s['seconds'] for s in rows if s['mutual'])
    summaries.append({'pair':pair,'radius':radius,'baseline_median_seconds':before,'candidate_median_seconds':after,'baseline_to_candidate_ratio':before/after})
  assert all(sha(p)==h for p,h in pins.items())
  report.update(status='complete_bounded_six_real_photo_mutual_controls',samples=samples,summaries=summaries,limits=['Same-process alternating real descriptor matcher controls only; shared-host costs are not universal throughput','No approximation, threshold or conservative work admission changes; exhaustive callback parity separately gated','Long all-query run remains on the previous frozen baseline; this experiment does not change its executable','No full native six-recipe collection speedup claim'])
 except Exception as error:report.update(status='failed_real_photo_mutual_controls',error=str(error));raise
 finally:save()
if __name__=='__main__':main()
