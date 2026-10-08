"""Exhaustive pair/order/pins/count predicates; errors/refusals remain explicit."""
import argparse,hashlib,json,math
from pathlib import Path
from dedup_jpeg_domain import oriented_dimensions
p=argparse.ArgumentParser();p.add_argument('report',type=Path);p.add_argument('output',type=Path);p.add_argument('--checkpoint',action='store_true');args=p.parse_args();assert not args.output.exists();h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();digest=h(args.report);r=json.loads(args.report.read_text());assert r['required_pairs']==229
assert r['status']=='complete' or args.checkpoint and r['status']=='running'
for path,pin in r['pins'].items():assert h(path)==pin,path
D=Path('docs/research');T=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');base=json.loads((D/'dedup-combined-domain-release-original-full.json').read_text());m=json.loads((T/'prepared.json').read_text());records={x['filename']:x for v in m['images'].values() for x in v};priority=['202502.jpg','212602.jpg','204702.jpg','200501.jpg'];expected=sorted(base['results'],key=lambda x:(priority.index(x['query']) if x['query'] in priority else len(priority),x['query']));assert len(expected)==229
if not args.checkpoint:assert len(r['results'])==229
assert len(r['results'])<=229;counts={'supported':0,'pixel_rejected':0,'no_geometry':0,'refused':0,'native_error':0};refusals={};seen=set()
for row,want in zip(r['results'],expected):
 for k in ['original','query','group_id']:assert row[k]==want[k]
 key=(row['original'],row['query']);assert key not in seen;seen.add(key)
 source=T/'original-resolution-negative-200201'/row['original'];target=T/'original-resolution-strong-all'/row['query'];sd=oriented_dimensions(source,records[row['original']]['source_size']);td=oriented_dimensions(target,records[row['query']]['source_size']);assert row['source_tolerance']==2*math.hypot(*sd)/math.hypot(*td)
 if row['returncode']!=0:counts['native_error']+=1;continue
 e=row['evidence'];assert e==json.loads(row['stdout'])
 if e['status'] in ['ok','no_geometry']:
  assert 1<=e['attempted_recipes']<=3 and e['smoothing_radii']==[[2,2],[4,0],[0,4]][e['attempted_recipes']-1]
  if e['status']=='no_geometry':assert e['attempted_recipes']==3
 if e['status']=='ok':
  assert len(e['matrix'])==3 and all(len(x)==3 and all(math.isfinite(v) for v in x) for x in e['matrix']);assert len(e['points'])<=28000 and all(len(x)==2 and all(len(p)==2 and all(math.isfinite(v) for v in p) for p in x) for x in e['points'])
  assert len(e['anchors'])==len(e['directions'])==2 and all(x>=10 for x in e['anchors']);predicates=[]
  for anchors,v in zip(e['anchors'],e['directions']):
   assert v['sites']==25*anchors and 0<=v['agreeing_pairs']<=v['informative_pairs']<=8*v['valid_sites']<=8*v['sites'];predicates.append(v['informative_pairs']>=1000 and v['valid_sites']/v['sites']>=.3 and v['agreeing_pairs']/v['informative_pairs']>=.9)
  assert e['supported']==all(predicates);counts['supported' if e['supported'] else 'pixel_rejected']+=1
 elif e['status']=='no_geometry':assert not e['supported'];counts['no_geometry']+=1
 else:
  assert e['status'].startswith('refusal:') and not e['supported'];counts['refused']+=1;refusals[e['status']]=refusals.get(e['status'],0)+1
assert sum(counts.values())==len(r['results']);assert h(args.report)==digest
snapshot=json.loads((D/'dedup-anchor-rank-driven-fallback-real-source.json').read_text());assert h(snapshot['binary'])==snapshot['binary_sha256']
for rel,pin in snapshot['file_hashes'].items():assert h(Path(snapshot['snapshot'])/rel)==pin
args.output.write_text(json.dumps({'status':'verified_rank_driven_full_checkpoint' if args.checkpoint else 'verified_rank_driven_full_complete','pairs':len(r['results']),'required_pairs':229,'counts':counts,'refusals':refusals,'pins':{str(p):h(p) for p in [args.report,Path(__file__),D/'dedup-anchor-rank-driven-fallback-real-source.json']},'scope':'Exact corpus pair order/input pins/source snapshot/resolution policy and native count support predicates; failures retained explicitly. Does not independently recompute all geometry/pixels or establish unrelated precision, identity, collection or all contract cases.'},indent=2)+'\n');print('verified',len(r['results']),counts)
