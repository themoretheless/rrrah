"""Five alternating rounds on self, two publisher copies and two foreign photo banks."""
import hashlib,json,os,re,statistics,subprocess,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research'
BASE=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
OUT=D/'dedup-gradient-mutual-hoisted-real-repeated-20261011.json'
GATE=D/'dedup-gradient-mutual-hoisted-generic-20261011.json'
MANIFEST=D/'dedup-gradient-mutual-hoisted-snapshot-20261011.json'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 assert not OUT.exists();gate=json.loads(GATE.read_text());assert gate['status']=='complete_bounded_frozen_gradient_mutual_generic_execution' and gate['tests']==205
 manifest=json.loads(MANIFEST.read_text());snapshot=Path(manifest['snapshot']);assert all(sha(snapshot/p)==h for p,h in manifest['file_hashes'].items())
 row=next(r for r in gate['results'] if r['suite']=='rrrah_dedup');binary=Path(row['binary']);assert sha(binary)==row['binary_sha256']
 paths=[BASE/'original-resolution-negative-200201/200100.jpg',BASE/'original-resolution-strong-all/200101.jpg',BASE/'original-resolution-strong-all/200102.jpg',BASE/'original-resolution-strong-all/200001.jpg',BASE/'original-resolution-strong-all/200302.jpg']
 prepared=BASE/'prepared.json';corpus=json.loads(prepared.read_text());groups={x['filename']:x['group_id'] for rows in corpus['images'].values() for x in rows};assert [groups[p.name] for p in paths]==[200100,200100,200100,200000,200300]
 pins={str(p):sha(p) for p in [binary,GATE,MANIFEST,prepared,Path(__file__),D/'dedup-gradient-mutual-real-20261011.json',*paths]}
 report={'status':'running','producer_pid':os.getpid(),'pins':pins,'scope':__doc__,'paths':list(map(str,paths)),'rounds':[]}
 def save():
  temp=OUT.with_suffix('.json.tmp');temp.write_text(json.dumps(report,indent=2)+'\n');temp.replace(OUT)
 save();environment=os.environ.copy();environment['RRRAH_GRADIENT_BENCH_FILES']=':'.join(map(str,paths));samples=[]
 try:
  for iteration in range(5):
   assert all(sha(p)==h for p,h in pins.items());process=subprocess.Popen([str(binary),'gradient::mutual_tests::real_photo_mutual_competitor_timing','--ignored','--nocapture'],env=environment,text=True,stdout=subprocess.PIPE,stderr=subprocess.PIPE);result={'round':iteration,'process_pid':process.pid,'status':'running'};report['rounds'].append(result);save();start=time.monotonic();stdout,stderr=process.communicate();result.update(returncode=process.returncode,elapsed_seconds=time.monotonic()-start,stdout=stdout,stderr=stderr)
   assert process.returncode==0 and '1 passed; 0 failed' in stdout
   current=[]
   for pair,n,k,radius,fast,count,seconds in re.findall(r'real_matcher pair=(\S+) features=(\d+):(\d+) radius=(\S+) mutual=(true|false) matches=(\d+) seconds=([0-9.]+)',stderr):current.append({'round':iteration,'pair':pair,'left_features':int(n),'right_features':int(k),'radius':radius,'candidate':fast=='true','matches':int(count),'seconds':float(seconds)})
   assert len(current)==40;samples+=current;result.update(status='passed',samples=current);save();print('round',iteration,'passed',flush=True)
  summaries=[]
  for pair in [f'0:{i}' for i in range(5)]:
   for radius in ['None','Some(64.0)']:
    selected=[s for s in samples if s['pair']==pair and s['radius']==radius];assert len(selected)==20 and len({s['matches'] for s in selected})==1
    for iteration in range(5):assert [s['candidate'] for s in selected if s['round']==iteration]==[False,True,True,False]
    before=[s['seconds'] for s in selected if not s['candidate']];after=[s['seconds'] for s in selected if s['candidate']];baseline=statistics.median(before);candidate=statistics.median(after)
    summaries.append({'pair':pair,'radius':radius,'matches':selected[0]['matches'],'samples_per_variant':10,'baseline_median_seconds':baseline,'candidate_median_seconds':candidate,'baseline_to_candidate_ratio':baseline/candidate,'baseline_range':[min(before),max(before)],'candidate_range':[min(after),max(after)]})
  assert all(sha(p)==h for p,h in pins.items());report.update(status='complete_bounded_ten_repeated_real_photo_controls',samples=samples,summaries=summaries,limits=['Original four-lane reference and candidate execute in the same copied program, five ABBA rounds per control','Shared-host timing variation remains; no universal or full native-collection throughput claim','Publisher-origin labels describe fixture relationship, not full-image identity','No pruning or threshold change; exact coordinate parity checked every invocation','Long all-query corpus and Linux baseline gates remain on their previous frozen source'])
 except Exception as error:report.update(status='failed_repeated_real_photo_controls',error=str(error));raise
 finally:save()
if __name__=='__main__':main()
