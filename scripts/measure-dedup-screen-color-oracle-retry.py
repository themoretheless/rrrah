"""External held-out color diagnostic on one fixed screen ROI; no acceptance oracle."""
import hashlib,json,math,sys
from pathlib import Path
import cv2,numpy as np
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');b=Path('docs/research');baseline=b/'dedup-screen-filter8.json';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();g=json.loads(baseline.read_text());assert all(h(k)==v for k,v in g['input_hashes'].items());matrix=np.asarray(g['evidence']['matrix'],dtype=np.float64);source=root/'original-resolution-negative-200201/200500.jpg';target=root/'original-resolution-strong-all/200501.jpg';negative=root/'original-resolution-negative-200201/200000.jpg';paths=[source,target,negative,baseline,Path(__file__),Path(sys.executable),*Path(cv2.__file__).parent.glob('*.so')];pins={str(p):h(p) for p in paths}
def read(p):
 image=cv2.imread(str(p),cv2.IMREAD_COLOR);assert image is not None;rgb=image[:,:,::-1].astype(np.float64)/255.;return np.where(rgb<=.04045,rgb/12.92,((rgb+.055)/1.055)**2.4)
def align(image):
 rh,rw=reference_shape[:2];ih,iw=image.shape[:2];model=matrix*np.asarray([rw/iw,rh/ih,1.])[None,:]
 warped=cv2.warpPerspective(image,model,(640,480),flags=cv2.INTER_LINEAR,borderMode=cv2.BORDER_CONSTANT,borderValue=0)
 mask=cv2.warpPerspective(np.ones(image.shape[:2],np.float64),model,(640,480),flags=cv2.INTER_LINEAR,borderMode=cv2.BORDER_CONSTANT,borderValue=0)
 return cv2.boxFilter(warped,-1,(17,17),normalize=True,borderType=cv2.BORDER_CONSTANT),cv2.boxFilter(mask,-1,(17,17),normalize=True,borderType=cv2.BORDER_CONSTANT)>.999999
reference_shape=read(source).shape
actual=cv2.boxFilter(read(target),-1,(17,17),normalize=True,borderType=cv2.BORDER_CONSTANT);assert actual.shape==(480,640,3)
y0,x0,hh,ww=60,240,60,80;yy,xx=np.indices((hh,ww));train=((xx//8+yy//8)%2)==0;results=[]
def diagonal(x,y):
 models=[]
 for i in range(3):
  s=x[:,i];t=y[:,i];ms=s.mean();mt=t.mean();var=np.mean((s-ms)**2);cov=np.mean((s-ms)*(t-mt));assert var>0
  candidates=[];aa=cov/var;bb=mt-aa*ms
  if .2<=aa<=5 and abs(bb)<=.1:candidates.append((aa,bb))
  for aa in [.2,5.]:candidates.append((aa,float(np.clip(mt-aa*ms,-.1,.1))))
  for bb in [-.1,.1]:candidates.append((float(np.clip(np.mean(s*(t-bb))/np.mean(s*s),.2,5.)),bb))
  models.append(min(candidates,key=lambda v:np.mean((v[0]*s+v[1]-t)**2)))
 return np.asarray(models)
for label,path in [('known_screen',source),('unrelated',negative),('synthetic_exact_aligned',source)]:
 expected,valid=align(read(path));x=expected[y0:y0+hh,x0:x0+ww];y=actual[y0:y0+hh,x0:x0+ww] if label!='synthetic_exact_aligned' else x.copy();valid=valid[y0:y0+hh,x0:x0+ww];fitting=valid&train;held=valid&~train;assert fitting.sum()>=1000 and held.sum()>=1000
 xt,yt=x[fitting],y[fitting];xv,yv=x[held],y[held];diag=diagonal(xt,yt);prediction=xv*diag[:,0]+diag[:,1];entry={'case':label,'training_samples':int(fitting.sum()),'heldout_samples':int(held.sum()),'training_source_variances':np.var(xt,axis=0).tolist(),'diagonal_coefficients':diag.tolist(),'diagonal_heldout_fraction':float(np.mean(np.max(np.abs(prediction-yv),axis=1)<=.03)),'diagonal_heldout_rmse':float(np.sqrt(np.mean((prediction-yv)**2)))}
 design=np.column_stack([xt,np.ones(len(xt))]);coeff,_,rank,singular=np.linalg.lstsq(design,yt,rcond=None);assert np.isfinite(coeff).all() and np.isfinite(singular).all();entry.update(affine_coefficients=coeff.tolist(),affine_rank=int(rank),affine_condition=float(singular[0]/singular[-1]))
 admitted=rank==4 and singular[0]/singular[-1]<=10000 and np.max(np.abs(coeff[:3]))<=5 and np.max(np.abs(coeff[3]))<=.1
 entry['affine_bounds_admitted']=bool(admitted)
 if admitted:
  prediction=np.einsum('ij,jk->ik',np.column_stack([xv,np.ones(len(xv))]),coeff,optimize=False);entry.update(affine_heldout_fraction=float(np.mean(np.max(np.abs(prediction-yv),axis=1)<=.03)),affine_heldout_rmse=float(np.sqrt(np.mean((prediction-yv)**2))))
 results.append(entry)
assert all(h(k)==v for k,v in pins.items());output=b/'dedup-screen-color-oracle-retry.json';assert not output.exists();output.write_text(json.dumps({'status':'complete_external_heldout_color_diagnostic','input_hashes':pins,'opencv_version':cv2.__version__,'numpy_version':np.__version__,'target_roi':[x0,y0,ww,hh],'filter_radius':8,'residual_tolerance':.03,'split':'Disjoint8x8 checker blocks; train parity0, holdout parity1','results':results,'scope':'One fixed known ROI, actual unrelated image with normalized source-domain mapping and synthetic exact aligned control. External OpenCV gamma/grayscale-independent color decoding and resampling differ from native oracle. Full affine bounds are diagnostic, no production acceptance or independent geometry truth.'},indent=2,allow_nan=False)+'\n');print(json.dumps([{k:v for k,v in r.items() if k not in ['diagonal_coefficients','affine_coefficients','training_source_variances']} for r in results]))
