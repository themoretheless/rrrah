#!/usr/bin/env python3
import hashlib,json,os,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 prepared=D/'dedup-disc21-heldout-negatives-prepared-20261012.json';audit=D/'dedup-disc21-heldout-negatives-prepared-audit-20261012.json';build=D/'dedup-disc21-heldout-negative-build-20261012.json'
 m=json.loads(prepared.read_text());a=json.loads(audit.read_text());b=json.loads(build.read_text())
 assert a['pairs']==16 and b['build_returncode']==0
 pins={str(p):sha(p) for p in [prepared,audit,build,Path(__file__)]}
 for report in [m,a,b]:
  for p,h in report['pins'].items():assert sha(p)==h,p
 s=json.loads((D/'dedup-disc21-heldout-negative-source-20261012.json').read_text())
 for p,h in s['file_hashes'].items():
  assert sha(Path(s['snapshot'])/p)==h
  if p.startswith('crates/rrrah-dedup/src/'):assert sha(R/p)==h,p
 exe=Path(b['executable']);raw=exe.read_bytes();assert raw[:4]==b'\xcf\xfa\xed\xfe' and int.from_bytes(raw[4:8],'little')==0x100000c
 old=json.loads((D/'dedup-disc21-exposure-recovery-api-20261012.json').read_text())['results'][0]['command'];weights=old[7];pins[weights]=sha(weights)
 report=D/'dedup-disc21-heldout-negatives-runtime-20261012.json';assert not report.exists()
 result={'status':'running','producer_pid':os.getpid(),'plan':m['plan'],'pins':pins,'results':[],'scope':'Fixed archived caller policy, public bidirectional CPU API. Hash-near publisher distractors excluded from prior local DISC21 reports; no semantic-hard-negative, pretraining holdout, general precision, full500 or full458 claim. Untagged linear sRGB caller assumption retained. Model memory outside managed budget.'}
 def save():
  tmp=report.with_suffix('.tmp');tmp.write_text(json.dumps(result,indent=2)+'\n');tmp.replace(report)
 save();images={x['id']:x for x in m['images']}
 for i,pair in enumerate(m['plan']):
  cmd=old[:4]+[str(exe),images[pair['query']]['path'],images[pair['reference']]['path'],weights,'640','2.0']
  row=dict(pair,status='running',command=cmd);result['results'].append(row)
  out=T/f'rrrah-disc21-heldout-negative-{i:02d}-20261012.stdout';err=out.with_suffix('.stderr');start=time.monotonic()
  with out.open('w') as o,err.open('w') as e:
   child=subprocess.Popen(cmd,cwd='/tmp',stdout=o,stderr=e);row['child_pid']=child.pid;save();rc=child.wait()
  row.update(returncode=rc,elapsed_seconds=time.monotonic()-start,stdout_path=str(out),stderr_path=str(err),stdout_sha256=sha(out),stderr_sha256=sha(err))
  try:
   assert rc==0
   evidence=json.loads(out.read_text());pairs=evidence['evidence']['pairs'];assert len(pairs)==1 and evidence['managed_used']==0
   row.update(status='complete',evidence=evidence,false_acceptance=bool(pairs[0]['supported']))
  except Exception as error:row.update(status='error',error=str(error))
  save();print(i+1,pair['query'],row['status'],row.get('false_acceptance'),flush=True)
 result['status']='complete_with_errors' if any(x['status']=='error' for x in result['results']) else 'complete';result['false_acceptances']=sum(x.get('false_acceptance',False) for x in result['results']);save()
if __name__=='__main__':main()
