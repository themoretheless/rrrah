#!/usr/bin/env python3
"""Audit a full pinned pair oracle against publisher origins, separately from n-ary parity."""
import argparse,ast,hashlib,json
from pathlib import Path
p=argparse.ArgumentParser()
for name in ('oracle','manifest','output'):p.add_argument(name,type=Path)
a=p.parse_args();helper=Path('scripts/verify-dedup-six-multifile-parity.py')
tree=ast.parse(helper.read_text());defs=[node for node in tree.body if isinstance(node,ast.FunctionDef) and node.name in ('exact_json','program_pins','load_reuse_source')]
assert len(defs)==3
import itertools
context={'json':json,'Path':Path,'hashlib':hashlib,'itertools':itertools,'reuse_sources':{},'active_sources':set()}
exec(compile(ast.Module(body=defs,type_ignores=[]),str(helper),'exec'),context)
r,rows=context['load_reuse_source'](str(a.oracle));m=json.loads(a.manifest.read_text());images={v['normalized_path']:v for group in m['images'].values() for v in group}
assert all(path in images for path in r['paths'])
for path in r['paths']:assert hashlib.sha256(Path(path).read_bytes()).hexdigest()==images[path]['normalized_sha256']
records=[]
for (left,right),row in sorted(rows.items()):
 e=row['evidence'];assert e['status']=='ok' and type(e['managed_used']) is int and e['managed_used']==0
 assert type(e['managed_peak']) is int and 0<=e['managed_peak']<=67108864
 assert type(e['candidate']) is bool and type(e['spatial_gradient']['candidate']) is bool
 supports=[]
 for lane in [e]+e['gradient_regions']+[e['spatial_gradient_regions']]:
  if lane is None:continue
  assert type(lane['region_support_count']) is int
  assert all(type(v['accepted_region']) is bool for v in lane['regions'])
  assert lane['region_support_count']==sum(v['accepted_region'] for v in lane['regions'])
  supports.append(lane['region_support_count'])
 whole=e['candidate'] or e['spatial_gradient']['candidate'];local=any(supports)
 records.append({'left':left,'right':right,'same_publisher_origin':images[r['paths'][left-1]]['group_id']==images[r['paths'][right-1]]['group_id'],'whole_candidate':whole,'region_supported':local,'whole_or_region':whole or local})
positives=[v for v in records if v['same_publisher_origin']];negatives=[v for v in records if not v['same_publisher_origin']]
digest=lambda path:hashlib.sha256(Path(path).read_bytes()).hexdigest()
a.output.write_text(json.dumps({'status':'verified_fixed_pair_labels','pairs':len(records),'publisher_copy_pairs':len(positives),'whole_copy_candidates':sum(v['whole_candidate'] for v in positives),'whole_or_region_copy_support':sum(v['whole_or_region'] for v in positives),'different_origin_pairs':len(negatives),'different_origin_flags':[v for v in negatives if v['whole_or_region']],'missed_copy_pairs':[v for v in positives if not v['whole_or_region']],'records':records,'input_hashes':{str(path):digest(path) for path in (a.oracle,a.manifest,helper,Path(__file__).resolve())},'scope':'All fixed two-source publisher-origin pairs, typed native evidence and recursive reuse provenance. No completed n-ary collection parity, semantic/burst precision or full-library coverage.'},indent=2)+'\n')
