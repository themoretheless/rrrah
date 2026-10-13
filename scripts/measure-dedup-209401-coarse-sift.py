"""Area-scale feature proposal diagnostic; never duplicate admission."""
import hashlib, json, math, time
from pathlib import Path
import cv2
import numpy as np
T = Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
files = [T/'original-resolution-negative-200201/209400.jpg', T/'original-resolution-strong-all/209401.jpg']
out = Path('docs/research/dedup-209401-coarse-sift.json')
assert not out.exists()
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
pins = {str(p):sha(p) for p in files+[Path(__file__)]}
cv2.setNumThreads(1)
images = [cv2.imread(str(p), cv2.IMREAD_GRAYSCALE) for p in files]
assert all(a is not None for a in images)
tolerance = 2*math.hypot(*images[0].shape)/math.hypot(*images[1].shape)
r = dict(status='running', pins=pins, opencv_version=cv2.__version__, original_shapes=[list(a.shape) for a in images], source_tolerance=tolerance, rows=[], scope='Sixteen independent area-scale SIFT proposal pairs. Original coordinate center mapping, separation>2, target2/source-scaled residuals unchanged. No native acceptance, whole-image model-domain validation, pixel confirmation or precision claim.')
def save():
    tmp=out.with_suffix('.json.tmp');tmp.write_text(json.dumps(r,indent=2)+'\n');tmp.replace(out)
save()
banks=[]
for a in images:
    bank={}
    for factor in [1,2,4,8]:
        h,w=a.shape; small=cv2.resize(a,(w//factor,h//factor),interpolation=cv2.INTER_AREA) if factor!=1 else a
        cv2.setRNGSeed(173)
        keys,desc=cv2.SIFT_create(nfeatures=5000,contrastThreshold=.001).detectAndCompute(small,None)
        sx=w/small.shape[1];sy=h/small.shape[0]
        points=[[(k.pt[0]+.5)*sx-.5,(k.pt[1]+.5)*sy-.5] for k in keys]
        bank[factor]=(points,desc,list(small.shape))
    banks.append(bank)
for sf in [1,2,4,8]:
    for tf in [1,2,4,8]:
        started=time.monotonic();cv2.setRNGSeed(173)
        ka,da,sa=banks[0][sf];kb,db,sb=banks[1][tf];matches=[]
        if da is not None and db is not None and len(db)>=2:
            for pair in cv2.BFMatcher().knnMatch(da,db,k=2):
                if len(pair)==2 and pair[0].distance<.75*pair[1].distance:
                    m=pair[0];matches.append((float(m.distance),ka[m.queryIdx],kb[m.trainIdx]))
        points=[]
        for _,left,right in sorted(matches):
            if all(math.dist(left,p[0])>2 and math.dist(right,p[1])>2 for p in points):points.append([left,right])
        row=dict(source_factor=sf,target_factor=tf,source_shape=sa,target_shape=sb,source_features=len(ka),target_features=len(kb),ratio_matches=len(matches),distinct_points=points,matrix=None,bidirectional_inliers=0,forward_inliers=0)
        if len(points)>=4:
            H,_=cv2.findHomography(np.float64([p[0] for p in points]),np.float64([p[1] for p in points]),cv2.RANSAC,2.,maxIters=20000,confidence=.999)
            if H is not None and np.all(np.isfinite(H)):
                try:
                    inverse=np.linalg.inv(H);row['matrix']=H.tolist()
                    def apply(m,p):
                        v=m@np.array([*p,1.]);return v[:2]/v[2]
                    for left,right in points:
                        f=np.linalg.norm(apply(H,left)-right)<=2;rcheck=np.linalg.norm(apply(inverse,right)-left)<=tolerance
                        row['forward_inliers']+=int(f);row['bidirectional_inliers']+=int(f and rcheck)
                except np.linalg.LinAlgError:
                    row['fit_error']='singular_model'
        row['elapsed_seconds']=time.monotonic()-started;r['rows'].append(row);save()
        print(sf,tf,len(points),row['bidirectional_inliers'],flush=True)
assert all(sha(p)==h for p,h in pins.items())
r['status']='complete_coarse_sift_diagnostic';save()
