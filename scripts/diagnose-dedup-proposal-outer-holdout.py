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
coords=np.memmap('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays/folded-native-mesh-grid-128m-forward.xy64',mode='r',dtype='<f8',offset=len(b'RRRAH-MESH-XY64-V1\n')+8,shape=(600,800,2))
proposal_rows=[row for row in r['rows'] if row['case']=='201700' and row['eligible_proposal']]
# Every outer center has at least one axis42+ from seed. Its samples/taps
# approach the seed no closer than33; training taps never exceed21.
centers=np.array([(x,y) for y in range(-48,49,6) for x in range(-48,49,6) if max(abs(x),abs(y))>=42]);neighbors=np.array([[0,0],[-6,0],[6,0],[0,-6],[0,6]]);taps=np.array([(x,y) for y in range(-3,4) for x in range(-3,4)]);positions=centers[:,None,None,:]+neighbors[None,:,None,:]+taps[None,None,:,:]
training_centers=np.array([(x,y) for y in range(-12,13,3) for x in range(-12,13,3)]);training_positions=training_centers[:,None,None,:]+neighbors[None,:,None,:]+taps[None,None,:,:]
training_set=set(map(tuple,training_positions.reshape(-1,2)));outer_set=set(map(tuple,positions.reshape(-1,2)));assert not training_set&outer_set
reports=[]
for row in proposal_rows:
 source=images['201700'];target=images['201702'];pos=positions+np.array(row['target']);xx=pos[:,:,:,0];yy=pos[:,:,:,1]
 inside=(xx>=0)&(yy>=0)&(xx<800)&(yy<600);cx=np.clip(xx,0,799);cy=np.clip(yy,0,599);mapped=coords[cy,cx]
 valid_centers=inside.all(axis=(1,2))&np.isfinite(mapped).all(axis=(1,2,3))
 # Keep exactly the same support for baseline and selected displacement.
 for offset in [[0,0],row['offset']]:
  shifted=mapped+np.array(offset);valid_centers&=((shifted[:,:,:,0]>=0)&(shifted[:,:,:,1]>=0)&(shifted[:,:,:,0]+1<source.shape[1])&(shifted[:,:,:,1]+1<source.shape[0])).all(axis=(1,2))
 xx=xx[valid_centers];yy=yy[valid_centers];mapped=mapped[valid_centers];tv=target[yy,xx].mean(axis=2);td=tv[:,1:]-tv[:,:1];informative=np.abs(td)>=.005;count=int(informative.sum());scores=[]
 for offset in [[0,0],row['offset']]:
  shifted=mapped+np.array(offset);sx=shifted[:,:,:,0];sy=shifted[:,:,:,1];ix=np.floor(sx).astype(int);iy=np.floor(sy).astype(int);tx=sx-ix;ty=sy-iy
  values=((source[iy,ix]*(1-tx)+source[iy,ix+1]*tx)*(1-ty)+(source[iy+1,ix]*(1-tx)+source[iy+1,ix+1]*tx)*ty).mean(axis=2);sd=values[:,1:]-values[:,:1];agree=(np.abs(sd)>=.005)&((sd>0)==(td>0))&informative;scores.append(float(agree.sum()/count) if count else None)
 reports.append({'target':row['target'],'offset':row['offset'],'outer_centers':int(valid_centers.sum()),'informative_pairs':count,'baseline_fraction':scores[0],'shifted_fraction':scores[1],'local_heldout_fraction':row['heldout_fraction']})
output.write_text(json.dumps({'status':'complete_spatially_disjoint_outer_proposal_diagnostic','rows':reports,'training_outer_target_taps_disjoint':True,'outer_minimum_axis_distance':33,'training_maximum_axis_distance':21,'report_sha256':hashlib.sha256(report.read_bytes()).hexdigest(),'verifier_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'scope':'Selected shifts assessed on outer target samples with no shared target taps; same geometric support for baseline/shifted. Source taps may overlap due to geometry/interpolation. Geometry proposal extrapolation diagnostic only, no classifier.'},indent=2)+'\n');print(json.dumps(reports,indent=2))
