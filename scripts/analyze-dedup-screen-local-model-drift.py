"""Pinned numerical transfer diagnostic; no image-match acceptance claim."""
import hashlib,json,math
from pathlib import Path
base=Path('docs/research')
names=['dedup-screen-filter8.json','dedup-screen-filter8-audit.json','dedup-screen-inlier-regional-geometry.json','dedup-screen-inlier-regional-geometry-audit.json','dedup-screen-inlier-regional-model-pixels.json','dedup-screen-inlier-regional-model-pixels-terminal-audit.json']
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
pins={str(base/n):h(base/n) for n in names};pins[str(Path(__file__))]=h(__file__)
for n in names:
 d=json.loads((base/n).read_text())
 assert all(h(k)==v for k,v in d.get('input_hashes',{}).items())
g=json.loads((base/names[0]).read_text())['evidence'];r=json.loads((base/names[2]).read_text());e=r['evidence']
def project(m,p):
 x,y=p;z=m[2][0]*x+m[2][1]*y+m[2][2];assert math.isfinite(z) and abs(z)>1e-12
 return [(m[i][0]*x+m[i][1]*y+m[i][2])/z for i in range(2)]
def inverse(m):
 a,b,c=m[0];d,e,f=m[1];g,h,i=m[2]
 adj=[[e*i-f*h,c*h-b*i,b*f-c*e],[f*g-d*i,a*i-c*g,c*d-a*f],[d*h-e*g,b*g-a*h,a*e-b*d]]
 det=a*adj[0][0]+b*adj[1][0]+c*adj[2][0];assert abs(det)>1e-12
 return [[x/det for x in row] for row in adj]
inv=inverse(g['matrix']);models=[]
for region in e['regions']:
 if region['matrix'] is None:continue
 x,y,w,hh=region['target_domain'];samples=[]
 for dy in [0,.5,1]:
  for dx in [0,.5,1]:
   p=[x+dx*w,y+dy*hh];src=project(inv,p)
   back=project(g['matrix'],src);assert math.dist(p,back)<1e-8
   local=project(region['matrix'],src)
   samples.append({'target':p,'global_inverse_source':src,'local_target':local,'drift_pixels':math.dist(p,local)})
 residuals=[]
 for idx in region['inliers']:
  point=g['correspondences'][r['global_point_indices'][idx]]
  residuals.append(math.dist(project(region['matrix'],point[0]),point[1]))
 assert max(residuals)<=2+1e-8
 models.append({'target_domain':region['target_domain'],'inliers':len(residuals),'maximum_inlier_residual':max(residuals),'maximum_domain_drift':max(s['drift_pixels'] for s in samples),'samples':samples})
assert len(models)==4 and all(h(k)==v for k,v in pins.items())
out=base/'dedup-screen-local-model-drift.json';assert not out.exists()
out.write_text(json.dumps({'status':'verified_numerical_model_transfer','input_hashes':pins,'models':models,'scope':'Nine deterministic fitting-domain samples per model compared to the global homography. Inlier residuals checked separately. Does not prove true geometry, pixel resampling, causation or match acceptance.'},indent=2)+'\n')
print(json.dumps([{'domain':m['target_domain'],'inliers':m['inliers'],'max_drift':m['maximum_domain_drift']} for m in models]))
