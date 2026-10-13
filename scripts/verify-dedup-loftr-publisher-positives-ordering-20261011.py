#!/usr/bin/env python3
"""Independently verify the captured prefix, not a full-goal completion claim."""
import collections,hashlib,json,math,re,sys
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp');B=T/'rrrah-dedup-copydays'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def pixel_proof(p):
 assert p['supported'] is True and len(p['anchors'])==2 and all(n>=10 for n in p['anchors']) and len(p['directions'])==2
 for v in p['directions']:assert v['sites']>0 and v['informative_pairs']>=1000 and v['valid_sites']/v['sites']>=.3 and v['agreeing_pairs']/v['informative_pairs']>=.9

def main():
 partial='--partial' in sys.argv;report_path=D/'dedup-loftr-publisher-positives-20261011.json';raw=report_path.read_bytes();r=json.loads(raw);assert all(sha(p)==h for p,h in r['pins'].items());manifest=json.loads(Path(r['source_manifest']).read_text());assert all(sha(Path(manifest['snapshot'])/f)==h for f,h in manifest['file_hashes'].items())
 data=json.loads((B/'prepared.json').read_text());files=sorted(data['images']['original'],key=lambda x:x['filename'])+sorted(data['images']['strong'],key=lambda x:x['filename']);pairs=[]
 corpus_report_path=D/'dedup-gradient-corpus-pyramid-index-20261011.json';corpus_report=json.loads(corpus_report_path.read_text());assert corpus_report['status']=='complete_bounded_normalized_corpus_index_oracle';assert all(sha(p)==h for p,h in corpus_report['pins'].items())
 corpus=corpus_report['results'][-1];assert corpus['name']=='all386' and sha(corpus['paths'])==corpus['paths_sha256'];assert Path(corpus['paths']).read_text().splitlines()==[v['normalized_path'] for v in files];assert sha(corpus['log'])==corpus['log_sha256'] and sha(corpus['descriptors_path'])==corpus['descriptors_sha256']

 minimums=T/'rrrah-gradient-corpus-pyramid-index-qualified-20261011/all386.minimum-distances.tsv'
 for line in minimums.read_text().splitlines():
  i,j,v=line.split('\t');i=int(i);j=int(j);v=float(v)
  if i<157<=j and files[i]['group_id']==files[j]['group_id']:pairs.append((i,j,v))
 pairs.sort(key=lambda row:(row[2]<=.1,-row[2],row[0],row[1]));assert len(pairs)==229;assert len(r['plan'])==r['required']==458
 for n,(i,j,distance) in enumerate(pairs):
  original=files[i];query=files[j];a=B/'original-resolution-negative-200201'/original['filename'];b=B/'original-resolution-strong-all'/query['filename']
  for offset in [0,1]:
   row=r['plan'][2*n+offset];left,right,ad,bd=(a,b,original['source_size'],query['source_size']) if offset==0 else (b,a,query['source_size'],original['source_size']);assert row['original_id']==i and row['query_id']==j and row['group_id']==original['group_id'];assert row['original']==str(a) and row['query']==str(b) and row['left']==str(left) and row['right']==str(right) and row['direction']==['forward','reverse'][offset];assert row['gradient_minimum_distance']==distance and row['radius_0_1_proposed']==(distance<=.1);assert math.isclose(row['source_tolerance'],2*math.hypot(*ad)/math.hypot(*bd),rel_tol=1e-12)
 assert sum(not v['radius_0_1_proposed'] for v in r['plan'])==192
 completed=r['completed'];assert 0<=completed<=458 and len(r['results']) in [completed,completed+1];counts=collections.Counter();missing_counts=collections.Counter();rows=[]
 for n,v in enumerate(r['results'][:completed]):
  plan=r['plan'][n];assert v['index']==v['plan_index']==n and v['status']!='running';assert sha(v['log'])==v['log_sha256'] and Path(v['log']).read_text()==v['stdout']+v['stderr'];cmd=v['command'];assert cmd[:3]==['/usr/bin/time','-l','/usr/bin/env'];assert cmd[6:] == ['640',str(plan['source_tolerance']),'model-first','-',plan['left'],plan['right']];assert sha(cmd[4])==r['pins'][cmd[4]] and sha(cmd[5])==r['pins'][cmd[5]]
  if v['returncode']:
   assert v['status']=='execution_failed'
  else:
   value=json.loads(v['stdout']);assert value['status']=='ok' and value['source_paths']==[plan['left'],plan['right']] and value['files']==[0,1] and value['proposal_pairs']==[] and len(value['pairs'])==1 and value['managed_used']==0 and value['managed_peak']<=512*1024*1024;pair=value['pairs'][0];assert (pair['left'],pair['right'])==(0,1);e=pair['evidence'];assert e==v['evidence'];supported=e['supported'];status='publisher_positive_supported' if supported is True else ('publisher_positive_rejected' if supported is False else 'publisher_positive_inconclusive');assert status==v['status'];assert v['rss_bytes']==int(re.search(r'(\d+)\s+maximum resident set size',v['stderr']).group(1))
   if supported is True:
    if e['terminal']=='model_tiles':
     proof=[x['evidence'] for x in e['attempts'] if x['evidence'].get('supported') is True];assert proof
     for p in proof:assert p['geometry_inliers']>=10;pixel_proof(p['pixels'])
    elif e['terminal']=='model':assert e['geometry_inliers']>=10;pixel_proof(e['pixels'])
    elif e['terminal']=='native':pixel_proof(e['pixels'])
    else:raise AssertionError('Unexpected evidence class')
   elif supported is None:assert e['terminal']=='inconclusive'
  counts[v['status']]+=1
  if not plan['radius_0_1_proposed']:missing_counts[v['status']]+=1
  rows.append({'index':n,'original_id':plan['original_id'],'query_id':plan['query_id'],'direction':plan['direction'],'radius_0_1_proposed':plan['radius_0_1_proposed'],'status':v['status']})
 if not partial:assert r['status']=='complete_bounded_publisher_positive_execution' and completed==458
 archive=T/'rrrah-loftr-publisher-positives-qualified-20261011/audits';archive.mkdir(exist_ok=True);captured=archive/f'ordering-prefix-{completed:04d}.json';audit=D/f'dedup-loftr-publisher-positives-ordering-prefix-{completed:04d}-audit-20261011.json';assert not captured.exists() and not audit.exists();captured.write_bytes(raw);result={'status':'verified_partial_prefix_not_completion' if completed<458 else 'verified_bounded_full_publisher_positive_execution','completed':completed,'required':458,'counts':dict(counts),'missing_proposal_counts':dict(missing_counts),'results':rows,'captured_report':str(captured),'captured_report_sha256':sha(captured),'verifier':str(Path(__file__)),'verifier_sha256':sha(Path(__file__)),'scope':'Independent plan/source/hash/command/raw-output and positive geometry/pixel audit. Published positives only; no full precision, unknown-resolution, hard RSS, platform or goal completion claim.'};audit.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:result[k] for k in ['status','completed','required','counts','missing_proposal_counts']}))
if __name__=='__main__':main()
