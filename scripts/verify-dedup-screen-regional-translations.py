"""Independent least-squares translation and point-membership audit."""
import argparse,hashlib,json,math
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('report',type=Path);p.add_argument('output',type=Path);a=p.parse_args();assert not a.output.exists()
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
r=json.loads(a.report.read_text());pins={str(a.report):h(a.report),str(Path(__file__)):h(__file__)}
assert r['status']=='complete_native_experiment' and type(r['returncode']) is int and r['returncode']==0
assert all(h(k)==v for k,v in r['input_hashes'].items())
def pinned(name):return next(Path(k) for k in r['input_hashes'] if Path(k).name==name)
baseline=json.loads(pinned('dedup-screen-filter8.json').read_text());assert all(h(k)==v for k,v in baseline['input_hashes'].items());g=baseline['evidence'];m=g['matrix'];assert r['global_point_indices']==g['inliers']
points=[g['correspondences'][i] for i in g['inliers']]
assert [[float(v) for v in line.split()] for line in pinned('dedup-screen-inlier-regional-points.txt').read_text().splitlines()]==[s+t for s,t in points]
assert [float(v) for v in pinned('dedup-screen-global-translation-anchor.txt').read_text().split()]==[v for row in m for v in row]
e=r['evidence'];assert json.loads(r['stdout'])==e and e['status']=='ok'
def project(mat,s):
 x,y=s;z=mat[2][0]*x+mat[2][1]*y+mat[2][2];assert math.isfinite(z) and abs(z)>1e-12
 return [(mat[i][0]*x+mat[i][1]*y+mat[i][2])/z for i in range(2)]
expected=[];summary=[]
for y in range(4):
 for x in range(4):
  domain=[x*160.,y*120.,160.,120.]
  ids=[i for i,(_,t) in enumerate(points) if domain[0]<=t[0]<domain[0]+160 and domain[1]<=t[1]<domain[1]+120]
  if len(ids)<10:continue
  expected.append((domain,ids))
assert len(expected)==len(e['regions'])==4
for row,(domain,ids) in zip(e['regions'],expected):
 assert row['target_domain']==domain and row['point_indices']==ids
 delta=[math.fsum(points[i][1][j]-project(m,points[i][0])[j] for i in ids)/len(ids) for j in range(2)]
 assert all(abs(v-w)<1e-10 for v,w in zip(delta,row['translation'])) and math.hypot(*delta)<=2
 adjusted=[[m[i][j]+delta[i]*m[2][j] for j in range(3)] for i in range(2)]+[m[2]]
 assert all(abs(v-w)<1e-10 for aa,bb in zip(adjusted,row['matrix']) for v,w in zip(aa,bb))
 inliers=[i for i in ids if math.dist(project(adjusted,points[i][0]),points[i][1])<=2]
 assert inliers==row['inliers'] and len(inliers)>=10
 summary.append({'target_domain':domain,'translation':delta,'inliers':len(inliers)})
assert all(h(k)==v for k,v in pins.items()) and all(h(k)==v for k,v in r['input_hashes'].items())
a.output.write_text(json.dumps({'status':'verified_native_screen_regional_translations','verified_models':4,'summary':summary,'input_hashes':pins,'scope':'Independently recomputed membership, least-squares means, translation composition and tolerance2 residuals against pinned global inliers. No pixel-match or ground-truth geometry claim.'},indent=2)+'\n')
