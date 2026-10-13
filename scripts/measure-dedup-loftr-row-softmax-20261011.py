#!/usr/bin/env python3
"""Fresh-process A/B memory controls on the frozen publisher positive."""
import hashlib,json,os,re,shutil,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[1]; T=Path('/Users/themoretheless/.codex/tmp'); D=R/'docs/research'
def sha(p): return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 parent_path=D/'dedup-scheduled-loftr-original-positive-20261011.json'; parent=json.loads(parent_path.read_text())
 manifest_path=D/'dedup-loftr-row-softmax-snapshot-20261011.json'; manifest=json.loads(manifest_path.read_text()); snapshot=Path(manifest['snapshot'])
 assert all(sha(snapshot/f)==h for f,h in manifest['file_hashes'].items())
 assert all(sha(p)==h for p,h in parent['pins'].items())
 archive=T/'rrrah-loftr-row-softmax-qualified-20261011'; assert not archive.exists();archive.mkdir()
 baseline=archive/'baseline'; candidate=archive/'candidate'
 shutil.copy2(T/'rrrah-scheduled-loftr-original-positive-qualified-20261011/scheduled_outcome_collection_files',baseline)
 shutil.copy2(T/'rrrah-dedup-exact-portfolio-20261011/debug/scheduled_outcome_collection_files',candidate)
 pins=dict(parent['pins']);pins.update({str(p):sha(p) for p in [parent_path,manifest_path,Path(__file__),baseline,candidate]})
 report={'status':'running','pins':pins,'source_manifest':str(manifest_path),'results':[],'scope':'One publisher positive, both directions, original pixels, model side640. Fresh-process RSS, no hard cap or full corpus claim.'}
 out=D/'dedup-loftr-row-softmax-20261011.json';assert not out.exists()
 def save():
  temp=out.with_suffix('.tmp');temp.write_text(json.dumps(report,indent=2)+'\n');temp.replace(out)
 save()
 try:
  for name,binary,direction in [('baseline_forward',baseline,0),('candidate_forward',candidate,0),('candidate_reverse',candidate,1),('baseline_reverse',baseline,1)]:
   assert all(sha(p)==h for p,h in pins.items())
   command=list(parent['results'][direction]['command']);command[4]=str(binary)
   start=time.monotonic();proc=subprocess.run(command,capture_output=True,text=True);elapsed=time.monotonic()-start
   log=archive/(name+'.log');log.write_text(proc.stdout+proc.stderr)
   row={'case':name,'command':command,'returncode':proc.returncode,'elapsed_seconds':elapsed,'log':str(log),'log_sha256':sha(log),'stdout':proc.stdout,'stderr':proc.stderr};report['results'].append(row);save()
   assert proc.returncode==0,proc.stderr
   value=json.loads(proc.stdout); reference=json.loads(parent['results'][direction]['stdout'])
   assert value['pairs']==reference['pairs'], 'geometry/pixel evidence drift'
   assert value['managed_used']==0 and value['source_paths']==reference['source_paths'] and value['proposal_pairs']==[]
   row['rss_bytes']=int(re.search(r'(\d+)\s+maximum resident set size',proc.stderr).group(1));row['managed_peak']=value['managed_peak'];row['evidence_equal_to_baseline']=True;save();print(name,row['rss_bytes'],flush=True)
  assert all(sha(p)==h for p,h in pins.items())
  report['status']='complete_bounded_original_positive_memory_comparison';save()
 except Exception as e:report.update(status='failed',error=str(e));save();raise
if __name__=='__main__':main()
