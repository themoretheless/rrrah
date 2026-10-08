"""Verify complete fixed-region rank diagnostic and retain all native refusals."""
import hashlib,json
from pathlib import Path
D=Path('docs/research');p=D/'dedup-main-occlusion-all-rank-regions.json';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
r=json.loads(p.read_text());assert r['status']=='complete' and len(r['results'])==r['required_regions']==216
assert all(h(k)==v for k,v in r['input_hashes'].items())
b=json.loads((D/'dedup-main-photometric-misses-radius8.json').read_text());expected=[(q,i) for q in ['214402.jpg','212602.jpg'] for i in range(108)];summary=[]
for row,(query,index) in zip(r['results'],expected):
 assert row['query']==query and row['region_index']==index
 e=next(v['evidence'] for v in b['results'] if v['query']==query);assert row['domains']==e['regions'][index]['domains']
 if row['returncode']:continue
 assert row['rank']==[json.loads(x) for x in row['stdout'].splitlines()]
 assert [(x['direction'],x['filter_radius']) for x in row['rank']]==[(d,f) for d in ('forward','reverse') for f in (0,3,8)]
 for x in row['rank']:
  rect=row['domains'][1 if x['direction']=='forward' else 0];assert x['sites']==rect[2]*rect[3] and 0<=x['valid_sites']<=x['sites'] and 0<=x['agreeing_pairs']<=x['informative_pairs']<=8*x['valid_sites']
 for f in (0,3,8):
  directions=[x for x in row['rank'] if x['filter_radius']==f]
  eligible=all(x['informative_pairs']>=1000 and x['valid_sites']/x['sites']>=.3 for x in directions)
  fraction=min(x['agreeing_pairs']/x['informative_pairs'] if x['informative_pairs'] else 0 for x in directions)
  summary.append({'query':query,'region_index':index,'filter_radius':f,'eligible':eligible,'minimum_agreement_fraction':fraction,'point9_rank_only':eligible and fraction>=.9})
result={'status':'verified_all_existing_occlusion_rank_region_diagnostic','regions':216,'native_refusals':sum(bool(x['returncode']) for x in r['results']),'best':{q:max((x for x in summary if x['query']==q and x['eligible']),key=lambda x:x['minimum_agreement_fraction']) for q in ['214402.jpg','212602.jpg']},'rank_only_point9_count':sum(x['point9_rank_only'] for x in summary),'summary':summary,'pins':{str(x):h(x) for x in [p,Path(__file__)]},'scope':'Complete108 existing regions per pair, unchanged models and filters. Informative-count and coverage predicates explicitly measured. No rank-only copy admission, broad precision or independent pixel oracle for new pairs.'}
(D/'dedup-main-occlusion-all-rank-regions-audit.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:result[k] for k in ['regions','native_refusals','best','rank_only_point9_count']}))
