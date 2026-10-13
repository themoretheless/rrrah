#!/usr/bin/env python3
"""All229 published positive pairs, both directions, original inputs.
Pairs absent from radius0.1 pyramid proposals run first. Empty schedule hints
exercise complete fallback. No semantic-negative precision claim.
"""
import hashlib,json,math,os,re,shutil,subprocess,time
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp');B=T/'rrrah-dedup-copydays';OUT=D/'dedup-loftr-publisher-positives-20261011.json';A=T/'rrrah-loftr-publisher-positives-qualified-20261011'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 assert not OUT.exists() and not A.exists();A.mkdir()
 prep=B/'prepared.json';data=json.loads(prep.read_text());files=sorted(data['images']['original'],key=lambda x:x['filename'])+sorted(data['images']['strong'],key=lambda x:x['filename']);assert len(files)==386
 minimums=T/'rrrah-gradient-corpus-pyramid-index-qualified-20261011/all386.minimum-distances.tsv';radius_audit=D/'dedup-gradient-corpus-pyramid-radius-audit-20261011.json';calibration=json.loads(radius_audit.read_text());assert calibration['publisher_original_query_pairs']==229;assert all(sha(p)==h for p,h in calibration['pins'].items())
 pairs=[]
 for line in minimums.read_text().splitlines():
  i,j,v=line.split('\t');i=int(i);j=int(j);v=float(v)
  if i<157<=j and files[i]['group_id']==files[j]['group_id']:pairs.append((i,j,v))
 assert len(pairs)==229 and sum(v>.1 for i,j,v in pairs)==96
 pairs.sort(key=lambda row:(row[2]<=.1,-row[2],row[0],row[1]))
 report_path=D/'dedup-loftr-early-release-20261011.json';parent=json.loads(report_path.read_text());assert parent['status']=='complete_bounded_original_positive_memory_comparison';assert all(sha(p)==h for p,h in parent['pins'].items());mp=Path(parent['source_manifest']);manifest=json.loads(mp.read_text());snapshot=Path(manifest['snapshot']);assert all(sha(snapshot/f)==h for f,h in manifest['file_hashes'].items())
 # Root model differs from this measured binary only by qualified indentation.
 adoption_path=D/'dedup-loftr-early-release-adoption-20261011.json';adoption=json.loads(adoption_path.read_text());assert all(sha(R/f)==h for f,h in adoption['adopted'].items())
 for f in ['vendor/loftr/src/coarse_matching.rs','vendor/loftr/src/coarse_matching/tests.rs']:assert sha(R/f)==manifest['file_hashes'][f]
 binary=A/'scheduled_outcome_collection_files';shutil.copy2(T/'rrrah-loftr-early-release-qualified-20261011/candidate',binary)
 weights=B/'learned-matching-models/loftr-outdoor-native-20261010.safetensors';torch=T/'rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages/torch/lib'
 pins=dict(parent['pins']);pins.update({str(p):sha(p) for p in [prep,minimums,radius_audit,report_path,mp,adoption_path,binary,weights,Path(__file__)]})
 plan=[]
 for i,j,distance in pairs:
  original=files[i];query=files[j];left=B/'original-resolution-negative-200201'/original['filename'];right=B/'original-resolution-strong-all'/query['filename']
  for p,metadata in [(left,original),(right,query)]:assert sha(p)==metadata['source_sha256'];pins[str(p)]=metadata['source_sha256']
  for direction,a,b,ad,bd in [('forward',left,right,original['source_size'],query['source_size']),('reverse',right,left,query['source_size'],original['source_size'])]:plan.append({'original_id':i,'query_id':j,'group_id':original['group_id'],'original':str(left),'query':str(right),'direction':direction,'left':str(a),'right':str(b),'source_tolerance':2*math.hypot(*ad)/math.hypot(*bd),'gradient_minimum_distance':distance,'radius_0_1_proposed':distance<=.1})
 assert len(plan)==458 and sum(not row['radius_0_1_proposed'] for row in plan)==192
 report={'status':'running','producer_pid':os.getpid(),'source_manifest':str(mp),'pins':pins,'plan':plan,'required':458,'completed':0,'results':[],'scope':'All229 publisher positive pairs in both directions, original inputs, model side640, empty schedule hints; missing192 radius0.1 directions first. Untagged linear-sRGB diagnostic assumption retained. Frozen native source, not whole dirty workspace. Unknown, false, error remain explicit; no negative precision, format or platform claim.'}
 def save():
  tmp=OUT.with_suffix('.tmp');tmp.write_text(json.dumps(report,indent=2)+'\n');tmp.replace(OUT)
 save()
 try:
  for index,row in enumerate(plan):
   for p in [row['left'],row['right'],str(binary),str(weights)]:assert sha(p)==pins[p]
   command=['/usr/bin/time','-l','/usr/bin/env','DYLD_LIBRARY_PATH='+str(torch),str(binary),str(weights),'640',str(row['source_tolerance']),'model-first','-',row['left'],row['right']]
   result={'index':index,'plan_index':index,'status':'running','command':command};report['results'].append(result);save();start=time.monotonic();proc=subprocess.Popen(command,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);result['pid']=proc.pid;save();stdout,stderr=proc.communicate();log=A/f'{index:04d}.log';log.write_text(stdout+stderr);result.update(returncode=proc.returncode,elapsed_seconds=time.monotonic()-start,log=str(log),log_sha256=sha(log),stdout=stdout,stderr=stderr)
   for p in [row['left'],row['right'],str(binary),str(weights)]:assert sha(p)==pins[p]
   if proc.returncode:
    result['status']='execution_failed'
   else:
    value=json.loads(stdout);assert value['status']=='ok' and value['source_paths']==[row['left'],row['right']] and value['files']==[0,1] and value['proposal_pairs']==[] and len(value['pairs'])==1 and value['managed_used']==0
    pair=value['pairs'][0];assert (pair['left'],pair['right'])==(0,1);result['managed_peak']=value['managed_peak'];result['rss_bytes']=int(re.search(r'(\d+)\s+maximum resident set size',stderr).group(1));result['evidence']=pair['evidence'];supported=pair['evidence'].get('supported');result['status']='publisher_positive_supported' if supported is True else ('publisher_positive_rejected' if supported is False else 'publisher_positive_inconclusive')
   report['completed']=index+1;save();print(index,result['status'],row['direction'],row['group_id'],flush=True)
  assert all(sha(p)==h for p,h in pins.items());report['status']='complete_bounded_publisher_positive_execution';save()
 except Exception as e:report.update(status='failed',error=str(e));save();raise
if __name__=='__main__':main()
