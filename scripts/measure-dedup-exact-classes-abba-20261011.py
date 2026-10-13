#!/usr/bin/env python3
"""Alternating frozen baseline/candidate complete exact-collection probes."""
import hashlib,json,os,statistics,subprocess,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research'
OUT=D/'dedup-exact-classes-abba-20261011.json'
ARCHIVE=Path('/Users/themoretheless/.codex/tmp/rrrah-exact-classes-abba-qualified-20261011')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 assert not OUT.exists() and not ARCHIVE.exists();ARCHIVE.mkdir()
 sources={name:D/file for name,file in [('baseline','dedup-exact-collection-scaling-baseline-20261011.json'),('candidate','dedup-exact-classes-scaling-20261011.json')]}
 reports={name:json.loads(p.read_text()) for name,p in sources.items()}
 pins={str(p):sha(p) for p in [*sources.values(),Path(__file__)]}
 for name,r in reports.items():
  assert r['returncode']==r['build_returncode']==0
  pins.update(r['pins']);manifest=Path(r['source_manifest']);m=json.loads(manifest.read_text())
  assert all(sha(Path(m['snapshot'])/p)==h for p,h in m['file_hashes'].items())
  assert sha(r['binary'])==r['binary_sha256']
 def verify():assert all(sha(p)==h for p,h in pins.items())
 report={'status':'running','producer_pid':os.getpid(),'pins':pins,'runs':[],'order':['baseline','candidate','candidate','baseline'],'scope':'Alternating process-local probes, all copies, 8/32/64 files, 1 MiB/file; same copied frozen programs and measurement policy; no universal host/RSS/device-I/O claim'}
 def save():
  p=OUT.with_suffix('.json.tmp');p.write_text(json.dumps(report,indent=2)+'\n');p.replace(OUT)
 save()
 try:
  for index,name in enumerate(report['order']):
   verify();command=reports[name]['command'];row={'index':index,'variant':name,'status':'running','command':command};report['runs'].append(row);save();start=time.monotonic()
   p=subprocess.Popen(command,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);row['process_pid']=p.pid;save();stdout,stderr=p.communicate()
   log=ARCHIVE/(str(index)+'-'+name+'.log');log.write_text(stdout+stderr);row.update(returncode=p.returncode,log=str(log),log_sha256=sha(log),elapsed_seconds=time.monotonic()-start)
   assert p.returncode==0 and 'test result: ok. 1 passed; 0 failed;' in stdout,(stdout,stderr)
   rows=[json.loads(l.split(' ',1)[1]) for l in stdout.splitlines() if l.startswith('RRRAH_EXACT_SCALING ')]
   assert [r['files'] for r in rows]==[8,32,64]
   for r in rows:
    n=r['files'];pairs=n*(n-1)//2;assert r['pairs']==r['exact_pairs']==pairs and len(r['elapsed_ns'])==3
    reads=4*n+(6*pairs if name=='baseline' else 4*pairs+2*(n-1));assert r['analytical_source_bytes']==reads*r['file_bytes']
   row.update(status='passed',rows=rows);save();print(index,name,'passed',flush=True)
  controls=[]
  for n in [8,32,64]:
   samples={name:[v for run in report['runs'] if run['variant']==name for r in run['rows'] if r['files']==n for v in r['elapsed_ns']] for name in reports}
   assert all(len(v)==6 for v in samples.values());med={name:statistics.median(v)/1e9 for name,v in samples.items()}
   controls.append({'files':n,'samples_ns':samples,'median_seconds':med,'baseline_over_candidate':med['baseline']/med['candidate']})
  verify();report.update(status='complete_alternating_frozen_exact_classes_scaling',controls=controls,samples=36)
 except Exception as e:report.update(status='failed_alternating_exact_classes_scaling',error=str(e));raise
 finally:save()
 print(report['status'])
 for r in report['controls']:print(r['files'],r['median_seconds'],r['baseline_over_candidate'])
if __name__=='__main__':main()
