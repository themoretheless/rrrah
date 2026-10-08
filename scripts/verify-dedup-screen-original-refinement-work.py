"""Audit frozen refinement models and retained original feature residuals."""
import argparse,hashlib,json,math
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('report',type=Path);p.add_argument('output',type=Path);a=p.parse_args();assert not a.output.exists()
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();pins={str(a.report):h(a.report),str(Path(__file__)):h(__file__)};r=json.loads(a.report.read_text());assert r['status']=='complete_native_refinement' and type(r['returncode']) is int and r['returncode']==0 and r['required_models']==2 and all(h(k)==v for k,v in r['input_hashes'].items())
assert r['registration_policy']=={'radius':8,'stride':16,'rounds':16,'max_sample_pairs':180000000}
def pinned(name):return next(Path(k) for k in r['input_hashes'] if Path(k).name==name)
b=json.loads(pinned('dedup-screen-filter8.json').read_text());g=json.loads(pinned('dedup-sift-oracle-geometry.json').read_text());ga=json.loads(pinned('dedup-sift-oracle-geometry-audit.json').read_text());assert ga['status']=='verified_external_sift_native_geometry_controls' and all(h(k)==v for obj in [b,g,ga] for k,v in obj['input_hashes'].items());oracle_path=next(Path(k) for k in g['input_hashes'] if Path(k).name=='dedup-sift-oracle.json');oracle=json.loads(oracle_path.read_text());assert all(h(k)==v for k,v in oracle['input_hashes'].items())
screen=next(v for v in g['results'] if v['case']=='screen')['evidence']['regions'][0];models=[b['evidence']['matrix'],screen['matrix']];pointsets=[b['evidence']['correspondences'],[[v['source'],v['target']] for v in next(v for v in oracle['results'] if v['case']=='screen')['distinct_matches']]]
assert [[float(v) for v in line.split()] for line in pinned('dedup-screen-original-refinement-work-input.txt').read_text().splitlines()]==[[v for row in model for v in row] for model in models]
e=r['evidence'];assert json.loads(r['stdout'])==e and e['status']=='ok' and len(e['models'])==2;assert type(e['managed_used']) is int and e['managed_used']==0 and type(e['managed_peak']) is int and 0<=e['managed_peak']<=512*1024*1024;summary=[]
for row,initial,points in zip(e['models'],models,pointsets):
 assert row['initial']==initial
 if 'error' in row:
  assert type(row['error']) is str and row['error'] and 'refined' not in row;summary.append({'outcome':'native_refusal','error':row['error']});continue
 m=row['refined'];assert len(m)==3 and all(len(row)==3 and all(type(v) in [int,float] and math.isfinite(v) for v in row) for row in m)
 # Homographies have arbitrary scalar magnitude; normalize before determinant checks.
 assert m[2][2]!=0;m=[[v/m[2][2] for v in row] for row in m]
 aa,bb,cc=m[0];dd,ee,ff=m[1];gg,hh,ii=m[2];det=aa*(ee*ii-ff*hh)-bb*(dd*ii-ff*gg)+cc*(dd*hh-ee*gg);assert math.isfinite(det) and abs(det)>1e-12
 residuals=[]
 for source,target in points:
  x,y=source;den=m[2][0]*x+m[2][1]*y+m[2][2]
  residuals.append(None if abs(den)<=1e-12 else math.dist([(m[j][0]*x+m[j][1]*y+m[j][2])/den for j in range(2)],target))
 ids=[i for i,v in enumerate(residuals) if v is not None and math.isfinite(v) and v<=2];summary.append({'outcome':'refined' if len(ids)>=10 else 'refined_insufficient_retained_geometry','retained_inliers':ids,'retained_inlier_count':len(ids),'correspondence_count':len(points)})
assert all(h(k)==v for k,v in pins.items()) and all(h(k)==v for k,v in r['input_hashes'].items())
a.output.write_text(json.dumps({'status':'verified_native_screen_refinement_models','input_hashes':pins,'verified_models':2,'summary':summary,'scope':'Typed refinement/refusal outcomes, exact starting models, resource release, finite nonsingular matrices and original feature residuals. Does not independently recompute objective or establish pixel acceptance.'},indent=2)+'\n')
