"""External SIFT proposal oracle with exact and unrelated controls; no duplicate acceptance."""
import hashlib,json,math,sys
from pathlib import Path
import cv2,numpy as np
from dedup_jpeg_domain import oriented_dimensions
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');b=Path('docs/research');manifest=root/'prepared.json';m=json.loads(manifest.read_text());h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
original=root/'original-resolution-negative-200201';strong=root/'original-resolution-strong-all';cases=[('exact',original/'200300.jpg',original/'200300.jpg'),('wrinkled',original/'200300.jpg',strong/'200302.jpg'),('unrelated',original/'200000.jpg',strong/'200302.jpg'),('screen',original/'200500.jpg',strong/'200501.jpg')]
files=[manifest,Path(__file__),Path(sys.executable),Path(cv2.__file__),Path('scripts/dedup_jpeg_domain.py'),*[p for _,a,c in cases for p in [a,c]],*Path(cv2.__file__).parent.glob('*.so')];pins={str(p):h(p) for p in files};out=b/'dedup-sift-lowcontrast-oracle.json';assert not out.exists();r={'status':'running_cases','input_hashes':pins,'contrast_threshold':0.01,'opencv_version':cv2.__version__,'numpy_version':np.__version__,'opencv_build_info':cv2.getBuildInformation(),'required_cases':4,'results':[],'scope':'Independent OpenCV SIFT on grayscale oriented original-resolution JPEG; nfeatures8000, ratio0.8, both locations separated>2. Exact and unrelated controls, known wrinkled and screen queries. Candidate evidence only; gamma/recipe differ from native linear gradient. No pixels, library integration or measured superiority.'}
def save():
 tmp=out.with_suffix('.tmp');tmp.write_text(json.dumps(r,indent=2)+'\n');tmp.replace(out)
def read(p):
 split='strong' if p.parent==strong else 'original';row=next(v for v in m['images'][split] if v['filename']==p.name);assert h(p)==row['source_sha256'];width,height=oriented_dimensions(p,row['source_size']);assert width*height<=6400000
 image=cv2.imread(str(p),cv2.IMREAD_GRAYSCALE);assert image is not None and image.shape==(height,width);return image
cv2.setNumThreads(1);cv2.setRNGSeed(0x1234abcd);sift=cv2.SIFT_create(nfeatures=8000,contrastThreshold=0.01);save()
for label,left,right in cases:
 assert all(h(k)==v for k,v in pins.items());ka,da=sift.detectAndCompute(read(left),None);kb,db=sift.detectAndCompute(read(right),None);assert len(ka)<=8500 and len(kb)<=8500
 raw=[]
 if da is not None and db is not None and len(db)>=2:
  for pair in cv2.BFMatcher(cv2.NORM_L2).knnMatch(da,db,k=2):
   if len(pair)==2 and pair[0].distance<.8*pair[1].distance:
    a,c=pair;raw.append({'source':list(ka[a.queryIdx].pt),'target':list(kb[a.trainIdx].pt),'distance':a.distance,'second_distance':c.distance})
 selected=[]
 for row in sorted(raw,key=lambda v:(v['distance'],v['source'],v['target'])):
  if all(all(math.dist(row[k],old[k])>2 for k in ['source','target']) for old in selected):selected.append(row)
 assert all(h(k)==v for k,v in pins.items());r['results'].append({'case':label,'left':str(left),'right':str(right),'feature_counts':[len(ka),len(kb)],'ratio_proposals':len(raw),'distinct_matches':selected});save()
r['status']='complete_external_candidate_oracle';save();print(json.dumps([{'case':v['case'],'features':v['feature_counts'],'proposals':len(v['distinct_matches'])} for v in r['results']]))
