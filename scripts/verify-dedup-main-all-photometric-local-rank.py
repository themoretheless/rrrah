"""Independent complete-region witness accounting and actual cached API predicates."""
import argparse,hashlib,json
from pathlib import Path
import numpy as np
p=argparse.ArgumentParser();p.add_argument('report',type=Path);p.add_argument('output',type=Path);p.add_argument('--checkpoint',action='store_true');args=p.parse_args();assert not args.output.exists();h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();report_digest=h(args.report);r=json.loads(args.report.read_text());assert r['required_cases']==16 and (r['status']=='complete' or args.checkpoint and r['status']=='running');assert all(h(k)==v for k,v in r['input_hashes'].items()) and all(h(k)==v for k,v in r['generated_hashes'].items());D=Path('docs/research');analysis=json.loads((D/'dedup-main-problem-refusal-analysis.json').read_text());expected=[v for v in analysis['rows'] if v['reason']=='photometric'];base=json.loads((D/'dedup-combined-domain-release-original-full.json').read_text());assert len(r['results'])<=16
if not args.checkpoint:assert len(r['results'])==16
summary=[]
for row,case in zip(r['results'],expected):
 assert row['query']==case['query'] and row['original']==case['original'];e=next(v['evidence'] for v in base['results'] if v['query']==row['query']);m=np.array(e['matrix']);inverse=np.linalg.inv(m);points=np.array(e['correspondences']);source=points[:,0,:];target=points[:,1,:];q=np.column_stack([source[:,0]*m[k,0]+source[:,1]*m[k,1]+m[k,2] for k in range(3)]);z=np.column_stack([target[:,0]*inverse[k,0]+target[:,1]*inverse[k,1]+inverse[k,2] for k in range(3)]);forward=np.linalg.norm(q[:,:2]/q[:,2:]-target,axis=1);reverse=np.linalg.norm(z[:,:2]/z[:,2:]-source,axis=1);assert np.all(np.isfinite(forward)) and np.all(np.isfinite(reverse));assert np.isclose(row['source_tolerance'],2*np.hypot(*row['dimensions'][0])/np.hypot(*row['dimensions'][1]));ids_by_region=[]
 for region in e['regions']:
  mask=(forward<=2)&(reverse<=row['source_tolerance'])
  for coords,rect in zip([source,target],region['domains']):x,y,w,height=rect;mask&=(coords[:,0]>=x)&(coords[:,0]<x+w)&(coords[:,1]>=y)&(coords[:,1]<y+height)
  ids_by_region.append(np.flatnonzero(mask).tolist())
 assert list(map(len,ids_by_region))==row['regional_witness_counts'];eligible=[i for i,ids in enumerate(ids_by_region) if len(ids)>=10];assert [v['region_index'] for v in row['regions']]==eligible;supported=0;native_errors=refusals=0
 for entry in row['regions']:
  assert entry['witness_indices']==ids_by_region[entry['region_index']]
  if entry['returncode']:native_errors+=1;continue
  assert [json.loads(line) for line in entry['stdout'].splitlines()]==entry['evidence'];assert [x['filter_radius'] for x in entry['evidence']]==[0,3,8]
  for x in entry['evidence']:
   if x['status']!='ok':refusals+=1;continue
   assert x['witnesses']==len(entry['witness_indices']);assert len(x['directions'])==2
   for count in x['directions']:assert 0<=count['valid_sites']<=count['sites'] and 0<=count['agreeing_pairs']<=count['informative_pairs']<=8*count['valid_sites']
   accepted=all(v['informative_pairs']>=1000 and v['valid_sites']/v['sites']>=.3 and v['agreeing_pairs']/v['informative_pairs']>=.9 for v in x['directions']);assert x['supported']==accepted
   if x['filter_radius']==8:supported+=int(accepted)
 assert supported==row['filter8_local_supported_regions'];summary.append({'query':row['query'],'proposed_regions':len(e['regions']),'geometry_eligible_regions':len(eligible),'filter8_supported_regions':supported,'native_errors':native_errors,'native_filter_refusals':refusals})
assert h(args.report)==report_digest
result={'status':'verified_all_photometric_cached_local_rank_checkpoint' if args.checkpoint else 'verified_all_photometric_cached_local_rank_terminal','verified_cases':len(summary),'required_cases':16,'locally_supported_cases':sum(v['filter8_supported_regions']>0 for v in summary),'summary':summary,'pins':{str(p):h(p) for p in [args.report,Path(__file__)]},'scope':'Independent NumPy geometry witnesses for every original proposed region, exact dispatch/order, native cached count bounds and support predicates. Native pixel comparison, shared normalization; broad independent pixel oracle, unrelated precision, file/collection admission remain pending.'};args.output.write_text(json.dumps(result,indent=2)+'\n');print(result['status'],len(summary),result['locally_supported_cases'])
