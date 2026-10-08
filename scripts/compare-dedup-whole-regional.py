#!/usr/bin/env python3
"""Pair pinned whole-image and regional evidence without conflating decisions."""
import argparse
import collections
import hashlib
import json
from pathlib import Path
p=argparse.ArgumentParser()
for key in ('whole','regional','output'):p.add_argument(key,type=Path)
a=p.parse_args()
raw={v:v.read_bytes() for v in (a.whole,a.regional)}
w,g=[json.loads(raw[v]) for v in (a.whole,a.regional)]
assert w['mode']=='complementary_gradient_portfolio' and g['mode']=='pyramid_region_grid'
assert w['manifest_sha256']==g['manifest_sha256']
for report in (w,g):
 assert report['required_pairs']==report['completed_pairs']==len(report['results'])==229
 assert len({row['query_id'] for row in report['results']})==229
 assert all(row['status']=='ok' and row['returncode']==0 for row in report['results'])
rows=[];counts=collections.Counter()
for whole,regional in zip(w['results'],g['results'],strict=True):
 assert whole['query_id']==regional['query_id'] and whole['label']==regional['label']=='publisher_origin_copy'
 e,re=whole['evidence'],regional['evidence']
 assert type(e['candidate']) is bool and e['managed_used']==re['managed_used_after_drop']==0
 assert all(type(region['accepted_region']) is bool for region in re['regions'])
 supported=sum(region['accepted_region'] for region in re['regions'])
 assert supported==re['region_support_count']
 outcome=('whole_and_regional' if supported else 'whole_only') if e['candidate'] else ('regional_only' if supported else 'neither')
 counts[outcome]+=1
 rows.append({'query_id':whole['query_id'],'whole_candidate':e['candidate'],'accepted_regions':supported,'outcome':outcome})
for path,data in raw.items():assert path.read_bytes()==data
out={'pairs':229,'counts':dict(sorted(counts.items())),'rows':rows,
     'input_sha256':{str(path):hashlib.sha256(data).hexdigest() for path,data in raw.items()},
     'scope':'Paired existing frozen reports, not an integrated library run. Regional support is local correspondence evidence and does not promote whole-image equality or semantic/burst duplicate decisions. Earlier individual source/pixel integrity audits remain required.'}
a.output.write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out['counts'],indent=2))
