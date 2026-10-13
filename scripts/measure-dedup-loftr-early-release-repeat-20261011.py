#!/usr/bin/env python3
import hashlib,json,re,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';P=D/'dedup-loftr-early-release-20261011.json'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 parent=json.loads(P.read_text());pins=dict(parent['pins']);pins.update({str(P):sha(P),str(Path(__file__)):sha(Path(__file__))});assert all(sha(p)==h for p,h in pins.items());out=D/'dedup-loftr-early-release-repeat-20261011.json';assert not out.exists();archive=Path('/Users/themoretheless/.codex/tmp/rrrah-loftr-early-release-repeat-qualified-20261011');assert not archive.exists();archive.mkdir();rows={r['case']:r for r in parent['results']};report={'status':'running','pins':pins,'results':[],'scope':'Repeated fresh-process ABBA forward, BAAB reverse; one positive pair, shared host, no general speed claim.'}
 def save():out.write_text(json.dumps(report,indent=2)+'\n')
 save()
 for direction,order in [('forward',['baseline','candidate','candidate','baseline']),('reverse',['candidate','baseline','baseline','candidate'])]:
  for i,variant in enumerate(order):
   ref=rows[variant+'_'+direction];start=time.monotonic();p=subprocess.run(ref['command'],capture_output=True,text=True);elapsed=time.monotonic()-start;log=archive/f'{direction}-{i}-{variant}.log';log.write_text(p.stdout+p.stderr);assert p.returncode==0;v=json.loads(p.stdout);assert v['pairs']==json.loads(ref['stdout'])['pairs'] and v['managed_used']==0;row={'direction':direction,'variant':variant,'elapsed_seconds':elapsed,'rss_bytes':int(re.search(r'(\d+)\s+maximum resident set size',p.stderr).group(1)),'log':str(log),'log_sha256':sha(log),'evidence_equal':True};report['results'].append(row);save();print(direction,variant,row['rss_bytes'],flush=True)
 assert all(sha(p)==h for p,h in pins.items());report['status']='complete_bounded_repeated_memory_comparison';save()
if __name__=='__main__':main()
