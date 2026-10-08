"""Verify full-match or global-inlier local native geometry diagnostics."""
import argparse,ast,hashlib,json,math
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('report',type=Path);p.add_argument('output',type=Path);a=p.parse_args();assert not a.output.exists()
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();pins={str(a.report):h(a.report),str(Path(__file__)):h(__file__)}
r=json.loads(a.report.read_text());assert r['status']=='complete_native_experiment' and type(r['returncode']) is int and r['returncode']==0 and all(h(p)==v for p,v in r['input_hashes'].items())
bp=Path('docs/research/dedup-screen-filter8.json');b=json.loads(bp.read_text());ba=Path('docs/research/dedup-screen-filter8-audit.json');audit=json.loads(ba.read_text());assert audit['input_hashes'][str(bp)]==h(bp) and all(h(p)==v for p,v in audit['input_hashes'].items());pins[str(ba)]=h(ba)
points=b['evidence']['correspondences']
if 'global_point_indices' in r:
 ids=r['global_point_indices'];assert ids==b['evidence']['inliers'] and all(type(v) is int for v in ids);points=[points[i] for i in ids]
else:assert len(points)==240
point_path=next(Path(k) for k in r['input_hashes'] if Path(k).suffix=='.txt');assert [[float(v) for v in line.split()] for line in point_path.read_text().splitlines()]==[[v for xy in pair for v in xy] for pair in points]
assert r['target_dimensions']==[640,480] and r['required_regions']==16
e=r['evidence'];assert e['status']=='ok' and json.dumps(e,sort_keys=True,allow_nan=False)==json.dumps(json.loads(r['stdout']),sort_keys=True,allow_nan=False) and len(e['regions'])==16
helper=Path('scripts/verify-dedup-scale-similarity-diagnostic.py');pins[str(helper)]=h(helper);node=next(v for v in ast.parse(helper.read_text()).body if isinstance(v,ast.FunctionDef) and v.name=='verify_model');ns={'math':math};exec(compile(ast.Module(body=[node],type_ignores=[]),str(helper),'exec'),ns)
summary=[]
for row,(y,x) in zip(e['regions'],[(y,x) for y in range(4) for x in range(4)]):
 rect=[x*160.,y*120.,160.,120.];assert row['target_domain']==rect
 ids=[i for i,p in enumerate(points) if rect[0]<=p[1][0]<rect[0]+160 and rect[1]<=p[1][1]<rect[1]+120];assert row['point_indices']==ids and all(type(v) is int for v in row['point_indices'])
 if row['matrix'] is None:assert row['inliers']==[]
 else:assert len(row['inliers'])>=10 and all(i in ids for i in row['inliers']);ns['verify_model'](row['matrix'],row['inliers'],points)
 summary.append({'target_domain':rect,'matches':len(ids),'inliers':len(row['inliers']),'has_model':row['matrix'] is not None})
assert all(h(p)==v for p,v in pins.items()) and all(h(p)==v for p,v in r['input_hashes'].items())
a.output.write_text(json.dumps({'status':'verified_native_screen_regional_geometry','verified_regions':16,'models':sum(v['has_model'] for v in summary),'summary':summary,'input_hashes':pins,'scope':'Exact target partitions, optional global-inlier selection and native min10/tol2residuals. No full-domain or pixel recovery claim.'},indent=2)+'\n')
