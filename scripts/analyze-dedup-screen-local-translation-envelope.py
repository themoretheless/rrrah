"""Finite per-region oracle envelope of nine measured native translations."""
import json,hashlib
from pathlib import Path
report=Path('docs/research/dedup-screen-translation-grid-retry.json');audit_path=Path('docs/research/dedup-screen-translation-grid-retry-audit.json');h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();r=json.loads(report.read_text());audit=json.loads(audit_path.read_text());assert audit['status']=='verified_native_screen_translation_pixel_grid' and audit['input_hashes'][str(report)]==h(report)
assert all(h(p)==v for doc in [r,audit] for p,v in doc['input_hashes'].items())
by_domain={};duplicates=0
for experiment in r['evidence']['experiments']:
 grouped={}
 for region in experiment['regions']:grouped.setdefault(tuple(region['domains'][1]),[]).append(region)
 for domain,regions in grouped.items():
  if len(regions)!=1:duplicates+=1;continue
  region=regions[0];pixels=region['pixels']
  if pixels is None:continue
  fraction=min(m/c if c else 0 for m,c,t in pixels['counts']);eligible=all(c>=1000 and c/t>=.3 for m,c,t in pixels['counts'])
  by_domain.setdefault(domain,[]).append({'offset':experiment['offset'],'fraction':fraction,'eligible_count_and_coverage':eligible,'accepted':region['accepted'],'source_domain':region['domains'][0]})
rows=[]
for domain,candidates in sorted(by_domain.items()):
 eligible=[v for v in candidates if v['eligible_count_and_coverage']]
 best=max(eligible,key=lambda v:v['fraction']) if eligible else None
 zero=next((v for v in candidates if v['offset']==[0.,0.]),None)
 rows.append({'target_domain':domain,'measured_offsets':len(candidates),'best_eligible':best,'zero':zero})
assert rows;accepted=sum(v['best_eligible'] is not None and v['best_eligible']['accepted'] for v in rows)
best=max((v for v in rows if v['best_eligible']),key=lambda v:v['best_eligible']['fraction'])
out=Path('docs/research/dedup-screen-local-translation-envelope.json');assert not out.exists();result={'status':'verified_finite_local_translation_envelope','target_regions':len(rows),'ambiguous_duplicate_domains_skipped':duplicates,'locally_accepted_regions':accepted,'best_eligible_region':best,'improved_over_zero':sum(v['best_eligible'] is not None and v['zero'] is not None and v['best_eligible']['fraction']>v['zero']['fraction'] for v in rows),'rows':rows,'input_hashes':{str(p):h(p) for p in [report,audit_path,Path(__file__)]},'scope':'Optimistic independent per-region selection among exactly nine measured translations. No new resampling or recovered model; source regions differ by translation. A zero result excludes only this finite envelope, not broader local/projective/nonrigid alignment.'};out.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:v for k,v in result.items() if k not in ['rows','input_hashes']},indent=2))
