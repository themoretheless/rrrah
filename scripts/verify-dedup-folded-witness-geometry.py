"""Independent native inlier-domain selection and unchanged-model proof."""
import argparse,hashlib,json,math
from pathlib import Path
parser=argparse.ArgumentParser();parser.add_argument('report',type=Path);parser.add_argument('output',type=Path);args=parser.parse_args();assert not args.output.exists()
digest=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
r=json.loads(args.report.read_text());assert r['status']=='verified_native_witness_geometry' and r['returncode']==0 and all(digest(k)==v for k,v in r['input_hashes'].items())
def pinned(name):return next(Path(k) for k in r['input_hashes'] if Path(k).name==name)
prior=pinned('dedup-folded-201702-grid8-geometry.json');p=json.loads(prior.read_text());audit_path=Path('docs/research/dedup-folded-201702-grid8-geometry-audit.json');audit=json.loads(audit_path.read_text());assert audit['status']=='verified_folded_crop_local_geometry' and audit['input_hashes'][str(prior)]==digest(prior)
assert all(digest(k)==v for k,v in audit['input_hashes'].items())
points=[[float(v) for v in line.split()] for line in pinned('dedup-folded-201702-regional-points.txt').read_text().splitlines()];assert len(points)==289 and all(len(v)==4 and all(math.isfinite(x) for x in v) for v in points)
e=r['evidence'];assert e==json.loads(r['stdout']) and len(e['regions'])==len(p['results'][0]['evidence']['regions'])==64
summary=[]
for index,(region,before) in enumerate(zip(e['regions'],p['results'][0]['evidence']['regions'])):
 assert {k:v for k,v in region.items() if k!='witness_domain'}==before
 if region['matrix'] is None:assert region['witness_domain'] is None;continue
 assert len(region['inliers'])>=10
 target=[points[i][2:] for i in region['inliers']];x,y,w,h=region['target_domain'];assert all(float(v).is_integer() for v in [x,y,w,h])
 assert all(x<=v[0]<x+w and y<=v[1]<y+h for v in target)
 low=[math.floor(min(v[i] for v in target)) for i in range(2)];high=[min([x+w,y+h][i],math.ceil(max(v[i] for v in target))+1) for i in range(2)]
 expected=[*low,int(high[0]-low[0]),int(high[1]-low[1])];assert region['witness_domain']==expected
 summary.append({'grid_index':index,'inliers':len(target),'fitting_domain':region['target_domain'],'witness_domain':expected})
assert len(summary)==r['models']==6
paths=[args.report,audit_path,Path(__file__)]
args.output.write_text(json.dumps({'status':'verified_native_witness_domains','models':6,'summary':summary,'input_hashes':{str(p):digest(p) for p in paths},'scope':'All64 native rows preserve prior residual-qualified model/inlier sets; six rectangles independently bound the same target points within original cells. No pixel or automatic full-search recovery proof.'},indent=2)+'\n')
