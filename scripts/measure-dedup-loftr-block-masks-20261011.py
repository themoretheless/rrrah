#!/usr/bin/env python3
import hashlib,json,os,re,shutil,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-loftr-block-masks-bench-20261011.json';A=T/'rrrah-loftr-block-masks-bench-qualified-20261011';assert not out.exists() and not A.exists();A.mkdir();mp=D/'dedup-loftr-block-masks-bench-harness-20261011.json';m=json.loads(mp.read_text());S=Path(m['harness']);assert all(sha(S/f)==h for f,h in m['files'].items());build=T/'dedup-loftr-block-masks-bench-build-20261011.jsonl';entries=[json.loads(line) for line in build.read_text().splitlines()];assert entries[-1].get('reason')=='build-finished' and entries[-1]['success'];bins=[e['executable'] for e in entries if e.get('reason')=='compiler-artifact' and e.get('executable')];assert len(bins)==1;binary=A/'mask_probe';shutil.copy2(bins[0],binary);pins={str(p):sha(p) for p in [mp,build,T/'dedup-loftr-block-masks-bench-build-20261011.log',binary,Path(__file__)]};report={'status':'running','producer_pid':os.getpid(),'pins':pins,'results':[],'scope':'Fresh CPU processes, deterministic 6400x6400 float confidence,2threads. Exact match-index parity and measured selector RSS/time; not full provider speed/RSS cap.'}
 def save():out.write_text(json.dumps(report,indent=2)+'\n')
 save()
 for repeat in range(3):
  for mode in ['dense','blocked']:
   env=os.environ.copy();env['DYLD_LIBRARY_PATH']=str(T/'rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib');env.pop('RRRAH_MASK_DENSE',None)
   if mode=='dense':env['RRRAH_MASK_DENSE']='1'
   cmd=['/usr/bin/time','-l',str(binary),'--exact','coarse_matching::tests::mask_memory_probe','--ignored','--nocapture','--test-threads=1'];start=time.monotonic();p=subprocess.run(cmd,env=env,capture_output=True,text=True);log=A/f'{repeat}-{mode}.log';log.write_text(p.stdout+p.stderr);assert p.returncode==0,p.stderr;values=re.search(r'MASK_INDICES=(\[[^\n]*\])',p.stdout);assert values;indices=json.loads(values[1]);rss=int(re.search(r'(\d+)\s+maximum resident set size',p.stderr)[1]);report['results'].append({'repeat':repeat,'mode':mode,'elapsed_seconds':time.monotonic()-start,'rss_bytes':rss,'indices':indices,'log':str(log),'log_sha256':sha(log)});save()
 assert all(x['indices']==report['results'][0]['indices'] for x in report['results']);assert all(sha(p)==h for p,h in pins.items());report['status']='complete_bounded_block_masks_selector_benchmark';save();print([(x['mode'],x['rss_bytes'],round(x['elapsed_seconds'],3)) for x in report['results']])
if __name__=='__main__':main()
