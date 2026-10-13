#!/usr/bin/env python3
"""Development diagnostic; Pillow JPEG pixels, not native acceptance evidence."""
import sys
sys.path.append('/Users/themoretheless/.codex/tmp/rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages')
import json,hashlib,math,os,time
from pathlib import Path
import numpy as np,torch,PIL
from PIL import Image
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();P=D/'dedup-aligned-sscd-diagnostic-plan-20261013.json';p=json.loads(P.read_text());assert p['producer_sha256']==sha(__file__);assert all(sha(k)==v for k,v in p['pins'].items());a=json.loads(Path(p['acquisition']).read_text());assert sha(a['weights'])==a['weights_sha256'];assert all(sha(Path(a['source'])/k)==v for k,v in a['source_sha256'].items());O=D/'dedup-aligned-sscd-diagnostic-20261013.json';assert not O.exists();d=dict(status='running',producer_pid=os.getpid(),plan_sha256=sha(P),rows=[],scope=p['scope']);start=time.monotonic()
def save():
 q=O.with_suffix('.tmp');q.write_text(json.dumps(d,indent=2)+'\n');q.replace(O)
def load(path):
 with Image.open(path) as im:
  assert im.format=='JPEG' and im.getexif().get(274,1)==1
  return np.array(im.convert('RGB'),dtype=np.float64)
def valid(image,x,y):
 return np.isfinite(x)&np.isfinite(y)&(x>=0)&(y>=0)&(x<image.shape[1]-1)&(y<image.shape[0]-1)
def sample(image,x,y,mask):
 x=np.where(mask,x,0);y=np.where(mask,y,0);ix=np.floor(x).astype(np.int64);iy=np.floor(y).astype(np.int64);fx=(x-ix)[...,None];fy=(y-iy)[...,None];v=image[iy,ix]*(1-fx)*(1-fy)+image[iy,ix+1]*fx*(1-fy)+image[iy+1,ix]*(1-fx)*fy+image[iy+1,ix+1]*fx*fy;return np.where(mask[...,None],v,127.5).astype(np.float32)/255
try:
 save();torch.set_num_threads(2);torch.manual_seed(173);model=torch.jit.load(a['weights'],map_location='cpu').eval();mean=torch.tensor([.485,.456,.406]).view(1,3,1,1);std=torch.tensor([.229,.224,.225]).view(1,3,1,1)
 d['runtime']=dict(python=sys.version,torch=torch.__version__,numpy=np.__version__,pillow=PIL.__version__,device='cpu',threads=2);save()
 with torch.inference_mode():
  for case in p['cases']:
   left=load(case['left']);right=load(case['right']);H=np.array(case['matrix']);inv=np.linalg.inv(H)
   for direction,A,B,M,region in [('forward',left,right,H,case['regions'][0]),('reverse',right,left,inv,case['regions'][1])]:
    x,y,w,h=region;assert w>0 and h>0;u,v=np.meshgrid(np.arange(320),np.arange(320));sx=x+(u+.5)*w/320-.5;sy=y+(v+.5)*h/320-.5;den=M[2,0]*sx+M[2,1]*sy+M[2,2];assert np.isfinite(den).all() and (np.abs(den)>1e-12).all();tx=(M[0,0]*sx+M[0,1]*sy+M[0,2])/den;ty=(M[1,0]*sx+M[1,1]*sy+M[1,2])/den;mask=valid(A,sx,sy)
    offsets=[[0,0],*p['offsets']]
    for dx,dy in offsets:mask &= valid(B,tx+dx,ty+dy)
    common=int(mask.sum());row=dict(label=case['label'],direction=direction,region=region,source_dimensions=[A.shape[1],A.shape[0]],target_dimensions=[B.shape[1],B.shape[0]],common_sites=common,total_sites=320*320,common_fraction=common/(320*320))
    if common==0:row['status']='no_common_support';d['rows'].append(row);save();continue
    pixels=[sample(A,sx,sy,mask),*[sample(B,tx+dx,ty+dy,mask) for dx,dy in offsets],np.where(mask[...,None],np.zeros((320,320,3),dtype=np.float32),np.full((320,320,3),.5,dtype=np.float32))];vectors=[];pixelpins=[]
    for pix in pixels:
     pixelpins.append(hashlib.sha256(pix.tobytes()).hexdigest());tensor=torch.from_numpy(pix).permute(2,0,1).unsqueeze(0);out=model((tensor-mean)/std).detach().cpu();assert tuple(out.shape)==(1,512) and torch.isfinite(out).all();vec=out[0].tolist();assert abs(math.sqrt(math.fsum(z*z for z in vec))-1)<1e-5;vectors.append(vec)
    scores=[math.fsum(x*y for x,y in zip(vectors[0],v)) for v in vectors[1:6]];row.update(status='scored',offsets=offsets,embeddings=vectors,pixel_tensor_sha256=pixelpins,baseline_cosine=scores[0],competitor_cosines=scores[1:],best_competitor_cosine=max(scores[1:]),margin=scores[0]-max(scores[1:]),source_mask_cosine=math.fsum(x*y for x,y in zip(vectors[0],vectors[6])),baseline_mask_cosine=math.fsum(x*y for x,y in zip(vectors[1],vectors[6])));d['rows'].append(row);save();print(case['label'],direction,round(row['margin'],4),round(row['baseline_cosine'],4),common,flush=True)
 d.update(status='complete_fixed_aligned_diagnostic',elapsed_seconds=time.monotonic()-start,pins_unchanged=all(sha(k)==v for k,v in p['pins'].items()));assert d['pins_unchanged'];save()
except Exception as e:
 d.update(status='terminal_failure',error_type=type(e).__name__,error=str(e));save();raise
