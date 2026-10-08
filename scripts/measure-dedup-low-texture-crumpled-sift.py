"""Independent feature-proposal diagnostic for low-texture crumpled print; not admission."""
import hashlib,json,math
from pathlib import Path
import cv2,numpy as np
T=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');source=T/'original-resolution-negative-200201'/'200300.jpg';target=T/'original-resolution-strong-all'/'200302.jpg';out=Path('docs/research/dedup-low-texture-crumpled-sift-diagnostic.json');assert not out.exists();h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();files=[source,target,Path(__file__)];before={str(p):h(p) for p in files};a=cv2.imread(str(source),cv2.IMREAD_GRAYSCALE);b=cv2.imread(str(target),cv2.IMREAD_GRAYSCALE);assert a is not None and b is not None;tol=2*math.hypot(a.shape[1],a.shape[0])/math.hypot(b.shape[1],b.shape[0]);rows=[];cv2.setNumThreads(1)
for enhanced,contrast in [(False,.04),(False,.001),(True,.04),(True,.001)]:
 cv2.setRNGSeed(173);clahe=cv2.createCLAHE(clipLimit=2.,tileGridSize=(8,8));aa=clahe.apply(a) if enhanced else a;bb=clahe.apply(b) if enhanced else b;sift=cv2.SIFT_create(nfeatures=5000,contrastThreshold=contrast);ka,da=sift.detectAndCompute(aa,None);kb,db=sift.detectAndCompute(bb,None);pairs=[]
 if da is not None and db is not None:
  for first,second in cv2.BFMatcher().knnMatch(da,db,k=2):
   if first.distance<.75*second.distance:pairs.append((float(first.distance),ka[first.queryIdx].pt,kb[first.trainIdx].pt))
 points=[]
 for _,left,right in sorted(pairs):
  if all(math.dist(left,p[0])>2 and math.dist(right,p[1])>2 for p in points):points.append([list(left),list(right)])
 row={'clahe':enhanced,'contrast_threshold':contrast,'source_features':len(ka),'target_features':len(kb),'ratio_matches':len(pairs),'distinct_points':points,'matrix':None,'forward_inliers':0,'bidirectional_inliers':0}
 if len(points)>=4:
  H,mask=cv2.findHomography(np.float64([p[0] for p in points]),np.float64([p[1] for p in points]),cv2.RANSAC,2.,maxIters=20000,confidence=.999)
  if H is not None and np.all(np.isfinite(H)):
   inverse=np.linalg.inv(H);row['matrix']=H.tolist()
   def apply(m,p):v=m@np.array([*p,1.]);return v[:2]/v[2]
   for left,right in points:
    f=np.linalg.norm(apply(H,left)-right)<=2;r=np.linalg.norm(apply(inverse,right)-left)<=tol;row['forward_inliers']+=int(f);row['bidirectional_inliers']+=int(f and r)
 rows.append(row)
assert all(h(p)==before[str(p)] for p in files);out.write_text(json.dumps({'status':'complete_independent_low_texture_feature_diagnostic','opencv_version':cv2.__version__,'source_tolerance':tol,'rows':rows,'pins':before,'scope':'OpenCV SIFT proposal reference, grayscale/CLAHE candidate variants,<=5000features per image, descriptor ratio.75 and2pixel target robust fit, original coordinate separation>2 both views. No native geometry domain/feature work proof, original-pixel confirmation, precision or Rust integration; acceptance thresholds not changed.'},indent=2)+'\n');print([(r['clahe'],r['contrast_threshold'],len(r['distinct_points']),r['forward_inliers'],r['bidirectional_inliers']) for r in rows])
