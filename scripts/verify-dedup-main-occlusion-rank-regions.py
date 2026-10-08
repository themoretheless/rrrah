"""Audit selection, provenance and bounds of occlusion rank diagnostic."""
import hashlib,json
from pathlib import Path
D=Path('docs/research');p=D/'dedup-main-occlusion-rank-regions.json'
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
r=json.loads(p.read_text());assert r['status']=='complete' and len(r['results'])==2
assert all(h(k)==v for k,v in r['input_hashes'].items())
b=json.loads((D/'dedup-main-photometric-misses-radius8.json').read_text());summary=[]
for row in r['results']:
 e=next(v['evidence'] for v in b['results'] if v['query']==row['query'])
 eligible=[(i,x) for i,x in enumerate(e['regions']) if x.get('pixels') and all(c[1]>=1000 and c[1]/c[2]>=.3 for c in x['pixels']['counts'])]
 index,region=max(eligible,key=lambda item:min(c[0]/c[1] for c in item[1]['pixels']['counts']))
 assert row['region_index']==index and row['domains']==region['domains'] and row['baseline_color_counts']==region['pixels']['counts'] and row['returncode']==0
 assert [json.loads(line) for line in row['stdout'].splitlines()]==row['rank']
 assert [(x['direction'],x['filter_radius']) for x in row['rank']]==[(d,f) for d in ('forward','reverse') for f in (0,3,8)]
 for x in row['rank']:
  rect=row['domains'][1 if x['direction']=='forward' else 0]
  assert x['sites']==rect[2]*rect[3] and 0<=x['valid_sites']<=x['sites']
  assert 0<=x['agreeing_pairs']<=x['informative_pairs']<=8*x['valid_sites']
 fractions=[min(x['agreeing_pairs']/x['informative_pairs'] for x in row['rank'] if x['filter_radius']==f) for f in (0,3,8)]
 summary.append({'query':row['query'],'region_index':index,'bidirectional_rank_fractions':dict(zip((0,3,8),fractions)),'reaches_unchanged_point9':any(v>=.9 for v in fractions)})
result={'status':'verified_occlusion_rank_diagnostic_provenance_and_selection','summary':summary,'pins':{str(x):h(x) for x in [p,Path(__file__)]},'scope':'Preselected best eligible color region, six rank directions each, source/executable hashes and evidence bounds. No independent pixel oracle for these new pairs; no copy-admission or causal-proof claim.'}
(D/'dedup-main-occlusion-rank-regions-audit.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(summary))
