#!/usr/bin/env python3
"""Pinned diagnostic only: SSCD TorchScript with explicit Pillow preprocessing."""
import sys
sys.path.append('/Users/themoretheless/.codex/tmp/rrrah-loftr-native-runtime-20261010/lib/python3.12/site-packages')
import json,hashlib,os,time,math
from pathlib import Path
import torch,numpy as np,PIL
from PIL import Image
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
P=D/'dedup-sscd-oracle-plan-20261013.json';plan=json.loads(P.read_text());A=Path(plan['acquisition']);a=json.loads(A.read_text());S=Path(a['source'])
assert sha(__file__)==plan['producer_sha256'] and sha(A)==plan['acquisition_sha256']
assert sha(a['weights'])==a['weights_sha256'] and all(sha(S/p)==h for p,h in a['source_sha256'].items())
assert all(sha(p)==h for p,h in plan['pins'].items())
O=D/'dedup-sscd-oracle-20261013.json';F=T/'rrrah-sscd-oracle-qualified-20261013';assert not O.exists() and not F.exists();F.mkdir()
d=dict(status='running',producer_pid=os.getpid(),plan_sha256=sha(P),required_images=len(plan['images']),completed_images=0,embeddings=[],scope=plan['scope'])
def save():
 q=O.with_suffix('.tmp');q.write_text(json.dumps(d,indent=2)+'\n');q.replace(O)
save();torch.set_num_threads(2);torch.manual_seed(173);model=torch.jit.load(a['weights'],map_location='cpu').eval();mean=torch.tensor([0.485,0.456,0.406],dtype=torch.float32).view(1,3,1,1);std=torch.tensor([0.229,0.224,0.225],dtype=torch.float32).view(1,3,1,1)
d['runtime']=dict(python=sys.version,torch=torch.__version__,numpy=np.__version__,pillow=PIL.__version__,device='cpu',threads=2);save();start=time.monotonic();vectors={}
with torch.inference_mode():
 for index,row in enumerate(plan['images']):
  path=Path(row['path']);assert path.stat().st_size<=plan['maximum_file_bytes'] and sha(path)==row['sha256']
  with Image.open(path) as source:
   assert source.format=='JPEG' and source.width*source.height<=plan['maximum_source_pixels'];dimensions=list(source.size);orientation=source.getexif().get(274,1)
   assert orientation==1, ('unexpected EXIF orientation',str(path),orientation)
   image=source.convert('RGB').resize((320,320),Image.Resampling.BILINEAR)
   pixels=np.array(image,dtype=np.uint8,copy=True);tensor=torch.from_numpy(pixels).permute(2,0,1).unsqueeze(0).to(dtype=torch.float32).div(255.)
   tensor=(tensor-mean)/std
  embedding=model(tensor).detach().cpu();assert tuple(embedding.shape)==(1,512) and torch.isfinite(embedding).all();values=embedding[0].tolist();norm=math.sqrt(sum(x*x for x in values));assert abs(norm-1)<1e-5
  assert sha(path)==row['sha256'];vectors[str(path)]=values;out=F/(str(index).zfill(4)+'.json');out.write_text(json.dumps(dict(path=str(path),input_sha256=row['sha256'],dimensions=dimensions,exif_orientation=orientation,embedding=values,norm=norm),indent=2)+'\n');d['embeddings'].append(dict(path=str(path),report=str(out),sha256=sha(out)));d['completed_images']+=1;save()
  if index%25==0:print('embedded',index+1,'/',len(plan['images']),flush=True)
rows=[]
for c in plan['pairs']:
 left=vectors[c['left']];right=vectors[c['right']];cosine=sum(x*y for x,y in zip(left,right));distance=sum((x-y)**2 for x,y in zip(left,right));assert math.isfinite(cosine) and math.isfinite(distance);rows.append(dict(c,cosine=cosine,squared_l2=distance,above_published_example_0_75=cosine>0.75))
# Ranking against all157 originals is explicitly a small no-distractor diagnostic.
originals=plan['originals'];rankings=[]
for c in plan['unique_positive_pairs']:
 scores=[(sum(x*y for x,y in zip(vectors[c['query']],vectors[p])),p) for p in originals];scores.sort(key=lambda v:(-v[0],v[1]));rank=next(i+1 for i,(_,p) in enumerate(scores) if p==c['original']);rankings.append(dict(query=c['query'],original=c['original'],rank=rank,top1=scores[0][1]))
d.update(status='complete_fixed_oracle',elapsed_seconds=time.monotonic()-start,rows=rows,rankings=rankings,top1=sum(r['rank']==1 for r in rankings),reciprocal_rank_mean=sum(1/r['rank'] for r in rankings)/len(rankings),inputs_unchanged=all(sha(p)==h for p,h in plan['pins'].items()),model_unchanged=sha(a['weights'])==a['weights_sha256'],source_unchanged=all(sha(S/p)==h for p,h in a['source_sha256'].items()));save();print('complete',len(rows),'pairs; top1',d['top1'],'/',len(rankings),flush=True)
