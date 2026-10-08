"""Audit a frozen checkpoint or complete combined origin-control gate."""
import argparse,hashlib,json
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('report',type=Path);p.add_argument('output',type=Path);p.add_argument('--checkpoint',action='store_true');a=p.parse_args();assert not a.output.exists();h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();digest=h(a.report);r=json.loads(a.report.read_text());assert r['status']=='complete' or a.checkpoint and r['status']=='running';assert r['required_pairs']==157 and r['required_negatives']==156
assert all(h(k)==v for k,v in r['input_hashes'].items()) and all(h(k)==v for k,v in r['generated_hashes'].items());manifest=next(Path(k) for k in r['input_hashes'] if Path(k).name=='prepared.json');m=json.loads(manifest.read_text());originals=m['images']['original'];expected=[v for v in originals if v['group_id']==r['query_group']]+[v for v in originals if v['group_id']!=r['query_group']];assert len(expected)==157
if not a.checkpoint:assert len(r['results'])==157
summary=[]
for row,original in zip(r['results'],expected):
 assert row['original']==original['filename'] and row['original_group']==original['group_id'] and row['positive_control']==(original['group_id']==r['query_group'])
 if row['candidate_returncode']:
  summary.append({'original':row['original'],'outcome':'candidate_error'});continue
 e=row['candidate'];assert e==json.loads(row['candidate_stdout']) and e['status']=='ok' and e['managed_used']==0 and e['managed_peak']<=512*1024*1024
 if e['matrix'] is None:assert not row['regions'] and not e['regions']
 else:assert [v['region_index'] for v in row['regions']]==row['eligible_region_indices']
 for region in row['regions']:
  assert region['prefilter_witnesses']>=10
  if not region['returncode']:
   assert [json.loads(x) for x in region['stdout'].splitlines()]==region['evidence'];assert [x['filter_radius'] for x in region['evidence']]==[0,3,8]
   for evidence in region['evidence']:
    if evidence['status']!='ok':continue
    assert evidence['witnesses']==region['prefilter_witnesses'];assert len(evidence['directions'])==2
    for x in evidence['directions']:assert 0<=x['valid_sites']<=x['sites'] and 0<=x['agreeing_pairs']<=x['informative_pairs']<=8*x['valid_sites']
    passes=all(x['valid_sites']/x['sites']>=.3 and x['informative_pairs']>=1000 and x['agreeing_pairs']/x['informative_pairs']>=.9 for x in evidence['directions']);assert evidence['supported']==passes
 support=any(x.get('supported',False) for region in row['regions'] for x in region.get('evidence',[]) if x['filter_radius']==8);assert support==row['local_rank8_supported']
 summary.append({'original':row['original'],'positive_control':row['positive_control'],'supported':support,'region_errors':sum(bool(x['returncode']) for x in row['regions'])})
assert h(a.report)==digest
result={'status':'verified_local_rank_origin_control_checkpoint' if a.checkpoint else 'verified_local_rank_origin_control_terminal','verified_pairs':len(summary),'required_pairs':157,'false_local_support':sum(x.get('supported',False) and not x.get('positive_control',False) for x in summary),'summary':summary,'pins':{str(p):h(p) for p in [a.report,Path(__file__)]},'scope':'Order/provenance, native memory, dispatched-region counts and support predicates. Region prefilter geometry independently checked only by native API for dispatched regions; complete exclusions and shared-decoder pixels need broader independent audit. Prefix incomplete, no promotion.'};a.output.write_text(json.dumps(result,indent=2)+'\n');print(result['status'],len(summary))
