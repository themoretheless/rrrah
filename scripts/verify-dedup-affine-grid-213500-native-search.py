"""Qualify no-model rejection only; a model requires further region auditing."""
import argparse,hashlib,json,math
from pathlib import Path
from dedup_jpeg_domain import oriented_dimensions
from dedup_distinct_points import verify_distinct_points
parser=argparse.ArgumentParser();parser.add_argument('report',type=Path);parser.add_argument('output',type=Path);args=parser.parse_args();assert not args.output.exists()
digest=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
report=json.loads(args.report.read_text());assert report['status']=='terminal' and report['returncode']==0 and all(digest(k)==v for k,v in report['input_hashes'].items())
source=Path(report['source']);query=Path(report['query']);assert source.name=='213500.jpg' and query.name=='200501.jpg'
prepared=next(Path(k) for k in report['input_hashes'] if Path(k).name=='prepared.json');manifest=json.loads(prepared.read_text());dims=[]
for path,split in [(source,'original'),(query,'strong')]:
 item=next(v for v in manifest['images'][split] if v['filename']==path.name);assert digest(path)==item['source_sha256'];dims.append(oriented_dimensions(path,item['source_size']))
e=report['evidence'];assert e==json.loads(report['stdout']) and e['status']=='ok' and e['managed_used']==0 and 0<=e['managed_peak']<=512*1024*1024
points=e['detail']['correspondences'];assert len(points)==e['points']<=28000
for point in points:
 for key,size in [('source',dims[0]),('target',dims[1])]:
  assert len(point[key])==2 and all(type(v) in [float,int] and math.isfinite(v) and 0<=v<size[i] for i,v in enumerate(point[key]))
verify_distinct_points([[point['source'],point['target']] for point in points])
assert e['matrix'] is None,'Native model present: rejection is unproven; audit its regions separately.'
assert e['inliers']==e['regions']==e['supports']==0 and e['detail']['inlier_ids']==e['detail']['regions']==[]
for original,record in report['qualified_source_provenance'].items():assert digest(record['preserved_path'])==record['qualified_sha256']
paths=[args.report,Path(__file__),Path(__file__).with_name('dedup_distinct_points.py'),Path(__file__).with_name('dedup_jpeg_domain.py')]
args.output.write_text(json.dumps({'status':'verified_native_no_model_rejection','points':len(points),'inliers':0,'regions':0,'supports':0,'input_hashes':{str(p):digest(p) for p in paths},'scope':'One unrelated color-only support rejected by fresh native geometry at the preserved binary revision. All exported point bounds/distinctness and absence of model/regions verified; not all-query or current source-footprint precision.'},indent=2)+'\n')
