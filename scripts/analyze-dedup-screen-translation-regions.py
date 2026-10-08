"""Compare unique target-region diagnostics; do not infer pixel-resampling truth."""
import json,hashlib
from pathlib import Path
report=Path('docs/research/dedup-screen-translation-grid-retry.json');audit_path=Path('docs/research/dedup-screen-translation-grid-retry-audit.json');h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();r=json.loads(report.read_text());audit=json.loads(audit_path.read_text());assert audit['status']=='verified_native_screen_translation_pixel_grid' and audit['input_hashes'][str(report)]==h(report)
assert all(h(p)==v for doc in [r,audit] for p,v in doc['input_hashes'].items())
rows=r['evidence']['experiments'];zero=next(v for v in rows if v['offset']==[0.,0.]);shifted=next(v for v in rows if v['offset']==[-1.,0.])
def measurement(region):
 p=region['pixels']
 if p is None:return None
 return {'minimum_matched_fraction':min(m/c if c else 0 for m,c,t in p['counts']),'minimum_coverage':min(c/t for m,c,t in p['counts']),'offset_components_at_bound':sum(abs(v)>=.1-1e-12 for d in p['offsets'] for v in d),'gains':p['gains'],'offsets':p['offsets'],'domains':region['domains'],'accepted':region['accepted']}
def unique(regions):
 grouped={}
 for row in regions:grouped.setdefault(tuple(row['domains'][1]),[]).append(row)
 return {k:v[0] for k,v in grouped.items() if len(v)==1}
a,b=unique(zero['regions']),unique(shifted['regions']);paired=[]
for domain in sorted(a.keys()&b.keys()):
 x,y=measurement(a[domain]),measurement(b[domain])
 if x is not None and y is not None:paired.append({'target_domain':domain,'zero_offset':x,'shifted':y})
assert paired;best=max(paired,key=lambda v:v['shifted']['minimum_matched_fraction']);out=Path('docs/research/dedup-screen-translation-regions.json');assert not out.exists()
result={'status':'verified_screen_unique_target_region_translation_comparison','paired_unique_target_regions':len(paired),'improved':sum(v['shifted']['minimum_matched_fraction']>v['zero_offset']['minimum_matched_fraction'] for v in paired),'worsened':sum(v['shifted']['minimum_matched_fraction']<v['zero_offset']['minimum_matched_fraction'] for v in paired),'best_shifted_region':best,'regions':paired,'input_hashes':{str(p):h(p) for p in [report,audit_path,Path(__file__)]},'scope':'Same target-domain unique regions, translated source domains may differ. Best region offset-bound saturation is diagnostic only, not causal calibration or local alignment truth; thresholds unchanged.'}
out.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:v for k,v in result.items() if k not in ['regions','input_hashes']},indent=2))
