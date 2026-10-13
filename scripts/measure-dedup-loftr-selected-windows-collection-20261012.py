#!/usr/bin/env python3
import hashlib,json,os,re,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 bp=D/'dedup-loftr-selected-windows-collection-build-20261012.json';b=json.loads(bp.read_text());assert b['status']=='complete_two_preserved_executables_not_executed' and all(sha(p)==h for p,h in b['pins'].items())
 sp=D/'dedup-loftr-selected-windows-stage-audit-20261012.json';s=json.loads(sp.read_text());assert all(sha(p)==h for p,h in s['pins'].items())
 prepared=D/'dedup-disc21-heldout-negatives-prepared-20261012.json';p=json.loads(prepared.read_text());assert all(sha(f)==h for f,h in p['pins'].items());pair=p['plan'][0];assert pair['query']=='Q36603' and pair['reference']=='R043984';images={x['id']:x for x in p['images']}
 old=json.loads((D/'dedup-disc21-exposure-recovery-api-20261012.json').read_text())['results'][0]['command'];pins=dict(b['pins']);pins.update({str(p):sha(p) for p in [bp,sp,prepared,Path(__file__)]});pins[old[7]]=sha(old[7])
 out=D/'dedup-loftr-selected-windows-collection-20261012.json';assert not out.exists();r={'status':'running','producer_pid':os.getpid(),'pins':pins,'pair':pair,'results':[],'scope':'One precommitted hash-near publisher distractor pair, current dedup and LoFTR root baseline versus selected-window candidate. Actual public bidirectional collection, fixed old caller policy/640/2.0. Fresh processes, other workload concurrent; not isolated timing, positive recall/full500/full458/hardcap or adoption qualification.'}
 def save():
  tmp=out.with_suffix('.tmp');tmp.write_text(json.dumps(r,indent=2)+'\n');tmp.replace(out)
 save()
 for build in b['builds']:
  mode=build['mode'];cmd=old[:4]+[build['executable'],images[pair['query']]['path'],images[pair['reference']]['path'],old[7],'640','2.0'];row={'mode':mode,'status':'running','command':cmd};r['results'].append(row);log=T/f'rrrah-loftr-selected-windows-collection-{mode}-20261012';stdout=log.with_suffix('.stdout');stderr=log.with_suffix('.stderr');start=time.monotonic()
  with stdout.open('w') as o,stderr.open('w') as e:
   child=subprocess.Popen(cmd,cwd='/tmp',stdout=o,stderr=e);row['child_pid']=child.pid;save();rc=child.wait()
  row.update(returncode=rc,elapsed_seconds=time.monotonic()-start,stdout=stdout.read_text(),stderr=stderr.read_text(),stdout_path=str(stdout),stderr_path=str(stderr));pins[str(stdout)]=sha(stdout);pins[str(stderr)]=sha(stderr);save();assert rc==0
  value=json.loads(row['stdout']);assert value['managed_used']==0 and len(value['evidence']['pairs'])==1;row.update(status='complete',evidence=value,rss_bytes=int(re.search(r'(\d+)\s+maximum resident set size',row['stderr'])[1]));save();print(mode,row['rss_bytes'],flush=True)
 assert r['results'][0]['evidence']==r['results'][1]['evidence'],'Complete evidence or managed-accounting drift'
 r['status']='complete_one_negative_pair_equivalent_not_adopted';save()
if __name__=='__main__':main()
