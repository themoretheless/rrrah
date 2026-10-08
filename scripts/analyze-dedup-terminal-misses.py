"""Separate observed refusal stages without changing admission thresholds."""
import hashlib,json
from pathlib import Path
report=Path('docs/research/dedup-combined-domain-release-original-full.json')
audit=Path('docs/research/dedup-combined-domain-release-original-full-terminal-audit.json')
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
r=json.loads(report.read_text());a=json.loads(audit.read_text());assert r['status']=='complete' and a['verified_rows']==229
assert a['input_hashes'][str(report)]==h(report)
rows=[]
for row in r['results']:
 e=row['evidence']
 if e['region_support_count']:continue
 records=[]
 for i,v in enumerate(e['regions']):
  if v['pixels'] is None:continue
  counts=v['pixels']['counts'];assert len(counts)==2
  for m,c,t in counts:assert 0<=m<=c<=t and t>0
  geometry_support=all(c>=1000 and c/t>=.3 for m,c,t in counts)
  agreement=min(m/c if c else 0 for m,c,t in counts)
  assert not (geometry_support and agreement>=.9)
  records.append({'region_index':i,'counts':counts,'minimum_agreement':agreement,'minimum_coverage':min(c/t for m,c,t in counts),'minimum_compared':min(c for m,c,t in counts),'support_before_photometric_gate':geometry_support,'domains':v['domains']})
 viable=[v for v in records if v['support_before_photometric_gate']]
 best=max(viable,key=lambda v:v['minimum_agreement'],default=None)
 rows.append({'original':row['original'],'query':row['query'],'correspondences':len(e['correspondences']),'inliers':len(e['inliers']),'reason':'geometry' if e['matrix'] is None else 'photometric' if viable else 'region_support','regions':len(e['regions']),'scored_regions':len(records),'support_eligible_regions':len(viable),'best_supported_region':best})
assert len(rows)==21
out={'status':'verified_refusal_stage_diagnostic','rows':rows,'counts':{key:sum(v['reason']==key for v in rows) for key in ['geometry','region_support','photometric']},'pins':{str(p):h(p) for p in [report,audit,Path(__file__)]},'scope':'Observed predicate failures only. Color/warp/blur mechanisms are not causally isolated; all admission gates unchanged.'}
Path('docs/research/dedup-main-problem-refusal-analysis.json').write_text(json.dumps(out,indent=2)+'\n')
print(out['counts'])
for v in sorted(rows,key=lambda v:(v['best_supported_region'] or {}).get('minimum_agreement',-1),reverse=True):
 print(v['query'],v['reason'],(v['best_supported_region'] or {}).get('minimum_agreement'))
