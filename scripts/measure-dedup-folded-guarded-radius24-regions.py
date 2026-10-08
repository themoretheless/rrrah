"""Four bounded cells per native local fold model; no extrapolated full grid."""
import hashlib,json,math,subprocess
from pathlib import Path
from dedup_jpeg_domain import oriented_dimensions
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');base=Path('docs/research')
digest=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
control=base/'dedup-guarded-roi-radius24-screen-control.json';c=json.loads(control.read_text());assert c['status']=='verified_known_roi_parity' and all(digest(k)==v for k,v in c['input_hashes'].items())
reference=base/'dedup-folded-201702-local-models-pixels.json';old=json.loads(reference.read_text());assert old['status']=='complete_native_experiment' and all(digest(k)==v for k,v in old['input_hashes'].items())
models=json.loads(old['stdout'])['experiments'];assert len(models)==8
exe=root/'affine-color-guarded-roi-radius24-probe';left=root/'original-resolution-negative-200201/201700.jpg';right=root/'original-resolution-strong-all/201702.jpg';prepared=root/'prepared.json';manifest=json.loads(prepared.read_text())
dims=[]
for path,split in [(left,'original'),(right,'strong')]:
 item=next(v for v in manifest['images'][split] if v['filename']==path.name);assert digest(path)==item['source_sha256'];dims.append(oriented_dimensions(path,item['source_size']))
assert digest(exe)==c['input_hashes'][str(exe)]
pins={str(p):digest(p) for p in [control,reference,exe,left,right,prepared,Path(__file__),Path(__file__).with_name('dedup_jpeg_domain.py')]}
output=base/'dedup-folded-guarded-radius24-regions.json';assert not output.exists();report={'status':'running','required_cases':32,'input_hashes':pins,'rows':[],'scope':'Eight prequalified local native models, each split into four target cells wholly within fitting domain. Supplied-model guarded file-region API; not automatic integrated piecewise search, per-cell feature proof or broad precision.'}
def save():
 temporary=output.with_suffix('.tmp');temporary.write_text(json.dumps(report,indent=2)+'\n');temporary.replace(output)
save()
for index,model in enumerate(models):
 h=model['matrix'];a,b,c0=h[0];d,e,f=h[1];g,j,k=h[2]
 adj=[[e*k-f*j,c0*j-b*k,b*f-c0*e],[f*g-d*k,a*k-c0*g,c0*d-a*f],[d*j-e*g,b*g-a*j,a*e-b*d]]
 det=a*adj[0][0]+b*adj[1][0]+c0*adj[2][0];assert math.isfinite(det) and det!=0;inverse=[[v/det for v in axis] for axis in adj]
 def project(x,y):
  den=inverse[2][0]*x+inverse[2][1]*y+inverse[2][2];assert den!=0
  return [(inverse[i][0]*x+inverse[i][1]*y+inverse[i][2])/den for i in range(2)]
 x,y,w,ht=map(int,model['fit_target_domain'])
 for gy in range(2):
  for gx in range(2):
   tx,ty=x+gx*w//2,y+gy*ht//2;tw,th=(gx+1)*w//2-gx*w//2,(gy+1)*ht//2-gy*ht//2
   corners=[project(xx,yy) for xx,yy in [(tx,ty),(tx+tw-1,ty),(tx,ty+th-1),(tx+tw-1,ty+th-1)]]
   lo=[max(0,min(dims[0][i],math.floor(min(v[i] for v in corners)))) for i in range(2)];hi=[max(0,min(dims[0][i],math.ceil(max(v[i] for v in corners))+1)) for i in range(2)]
   source=[*lo,hi[0]-lo[0],hi[1]-lo[1]];target=[tx,ty,tw,th];assert source[2]>0 and source[3]>0
   path=root/f'folded-guarded-radius24-region-{index}-{gx}-{gy}.txt';assert not path.exists();path.write_text(' '.join(map(str,[*[v for axis in h for v in axis],*source,*target]))+'\n');pin=digest(path)
   result=subprocess.run([str(exe),str(left),str(right),str(path)],capture_output=True,text=True);assert digest(path)==pin and all(digest(k)==v for k,v in pins.items())
   row={'model_index':index,'cell':[gx,gy],'matrix':h,'fit_target_domain':model['fit_target_domain'],'domains':[source,target],'input':str(path),'input_sha256':pin,'returncode':result.returncode,'stdout':result.stdout,'stderr':result.stderr}
   if result.returncode==0:
    evidence=json.loads(result.stdout);assert evidence['managed_used']==0;row.update(evidence=evidence,supported=evidence['status']=='ok' and len(evidence['directions'])==2 and all(v['matched']>=.9*v['samples'] for v in evidence['directions']))
   report['rows'].append(row);save();print(index,gx,gy,row.get('evidence',{}).get('status'),row.get('supported'),flush=True)
report['status']='complete_native_experiment';save()
