"""Independent direct per-window oracle for selected local proposal scores."""
import hashlib,json,sys
from pathlib import Path
import numpy as np
root=Path(__file__).resolve().parents[1];base=root/'docs/research'
report=Path(sys.argv[1]);output=Path(sys.argv[2]);r=json.loads(report.read_text());assert all(hashlib.sha256(Path(k).read_bytes()).hexdigest()==v for k,v in r['pins'].items())
d=json.loads((base/'dedup-rank-normalized-pixels.json').read_text());images={};magic=b'RRRAH-RANK-RGBA32-V1\n'
for stem in ['201700','201702','201900']:
 row=next(v for v in d['rows'] if Path(v['source']).stem==stem);path=Path(row['pixels']);assert hashlib.sha256(path.read_bytes()).hexdigest()==row['pixels_sha256']
 with path.open('rb') as f:assert f.read(len(magic))==magic;w=int.from_bytes(f.read(4),'little');h=int.from_bytes(f.read(4),'little')
 a=np.memmap(path,mode='r',dtype='<f4',offset=len(magic)+8,shape=(h,w,4));images[stem]=a[:,:,0].astype('f8')*.2126+a[:,:,1].astype('f8')*.7152+a[:,:,2].astype('f8')*.0722
grid_path=next(Path(k) for k in r['pins'] if k.endswith('-forward.xy64'))
coords=np.memmap(grid_path,mode='r',dtype='<f8',offset=len(b'RRRAH-MESH-XY64-V1\n')+8,shape=(600,800,2))
extent=r["policy"].get("center_extent",12);step=r["policy"].get("center_step",3);neighbor=r["policy"].get("neighbor_radius",6)
centers=np.array([(x,y) for y in range(-extent,extent+1,step) for x in range(-extent,extent+1,step)]);assert len(centers)==81
neighbors=np.array([[0,0],[-neighbor,0],[neighbor,0],[0,-neighbor],[0,neighbor]]);taps=np.array([(x,y) for y in range(-3,4) for x in range(-3,4)]);positions=centers[:,None,None,:]+neighbors[None,:,None,:]+taps[None,None,:,:];train=np.array([(i//9+i%9)%2==0 for i in range(81)])
for row in r['rows']:
 source=images[row['case']];target=images['201702'];pos=positions+np.array(row['target']);xx=pos[:,:,:,0];yy=pos[:,:,:,1];mapped=coords[yy,xx]+np.array(row['offset']);sx=mapped[:,:,:,0];sy=mapped[:,:,:,1];assert np.all(np.isfinite(mapped));ix=np.floor(sx).astype(int);iy=np.floor(sy).astype(int);tx=sx-ix;ty=sy-iy
 assert np.min(ix)>=0 and np.min(iy)>=0 and np.max(ix)+1<source.shape[1] and np.max(iy)+1<source.shape[0]
 values=((source[iy,ix]*(1-tx)+source[iy,ix+1]*tx)*(1-ty)+(source[iy+1,ix]*(1-tx)+source[iy+1,ix+1]*tx)*ty).mean(axis=2);tv=target[yy,xx].mean(axis=2);sd=values[:,1:]-values[:,:1];td=tv[:,1:]-tv[:,:1];informative=np.abs(td)>=.005;agree=(np.abs(sd)>=.005)&((sd>0)==(td>0))&informative
 for subset,name in [(train,'training'),(~train,'heldout')]:
  n=int(informative[subset].sum());assert n==row[name+'_pairs'];score=float(agree[subset].sum()/n);assert score==row[name+'_fraction'],(row['case'],row['target'],name,score,row[name+'_fraction'])
output.write_text(json.dumps({'status':'verified_direct_window_selected_proposal_scores','rows':len(r['rows']),'report_sha256':hashlib.sha256(report.read_bytes()).hexdigest(),'verifier_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'scope':'Exact selected training/heldout pair counts and fractions reproduced by direct49-tap means and independently evaluated bilinear samples. Does not verify argmax across all offsets, spatial holdout independence, mesh geometry, copy admission or broad precision.'},indent=2)+'\n');print('verified rows',len(r['rows']))
