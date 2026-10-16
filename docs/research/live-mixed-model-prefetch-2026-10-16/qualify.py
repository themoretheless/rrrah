import os,json,re,hashlib,subprocess,shutil
from pathlib import Path
root=Path(__file__).resolve().parents[3];out=Path(__file__).resolve().parent
binary=Path('/tmp/rrrah-alpha-isolated-target-20261016/debug/rrrah');identity=hashlib.sha256(binary.read_bytes()).hexdigest()
formats=[('stl','triangle.stl'),('obj','obj-negative-quad.obj'),('ply','ply-offset64-be.ply'),('off','off-concave-u.off')]
r={'status':'running','binary_sha256':identity,'cases':[],'scope':'actual native Metal EXR→model→EXR and mixed prefetch leases; authored subset fixtures; not materials, exact pixeloracle, calibrated HDR or broad3Dqualification'}
for ext,source in formats:
 folder=Path('/tmp/rrrah-mixed-model-prefetch-20261016')/ext;folder.mkdir(parents=True,exist_ok=True)
 for name,src in [('01-left.psd','psd-tools-gray0.psd'),('02-left.psb','psd-tools-gray0.psb'),('03-selected.exr','float-shifted-window-negative.exr'),('05-right.psb','psd-tools-gray0.psb'),('06-far.psd','psd-tools-gray0.psd')]:shutil.copyfile(root/'tests/fixtures/raster'/src,folder/name)
 target=folder/('04-target.'+ext);shutil.copyfile(root/'tests/fixtures/models'/source,target)
 env=dict(os.environ)
 for key in list(env):
  if key.startswith('RRRAH_QUALIFY_'):env.pop(key)
 env.update(RRRAH_GPU_BACKEND='metal',RRRAH_QUALIFY_GPU_RECOVERY='1',RRRAH_QUALIFY_RASTER_SWITCH=str(target),RRRAH_QUALIFY_RASTER_REQUIRE_CACHE='1',RRRAH_QUALIFY_MODEL_SWITCH='1',RRRAH_QUALIFY_PREFETCH_COUNT='3',RRRAH_QUALIFY_PREFETCH_ATTEMPTS='3')
 log=out/(ext+'.log')
 with log.open('w') as f:
  try:code=subprocess.run([str(binary),'--raster-cache-mb','32','--raster-cache-count','8','--model-cache-mb','16','--model-cache-count','4','--raster-swap-mb','0','--prefetch-previous','2','--prefetch-next','1',str(folder/'03-selected.exr')],env=env,cwd=root,stdout=f,stderr=subprocess.STDOUT,timeout=65).returncode
  except subprocess.TimeoutExpired:code=None
 s=log.read_text();groups={2:[],3:[],4:[]}
 for kind,g,path,held,attempt,count in re.findall(r'LIVE_(RASTER|MODEL)_PREFETCH_ATTEMPT generation=(\d+) path=(\S+) retained=(true|false) attempts=(\d+) retained_count=(\d+)',s):
  g=int(g)
  if g in groups:groups[g].append((kind,Path(path).name,held,int(attempt),int(count)))
 initial=[v[1] for v in groups[2]]
 valid=initial in [[target.name,'02-left.psb','01-left.psd'],['02-left.psb','01-left.psd',target.name]]
 valid=valid and [v[1] for v in groups[3]]==['05-right.psb','03-selected.exr','02-left.psb'] and [v[1] for v in groups[4]]==['02-left.psb',target.name,'05-right.psb']
 valid=valid and all(len(rows)==3 and [v[2:] for v in rows]==[('true',1,1),('true',2,2),('true',3,3)] for rows in groups.values())
 valid=valid and all(kind==('MODEL' if name==target.name else 'RASTER') for rows in groups.values() for kind,name,*_ in rows)
 nav=re.findall(r'LIVE_RASTER_NAV_PRESENT phase=(\d+) path=(\S+) generation=(\d+) previous_generation=(\d+) frames=(\d+) same_device=true gpu_wait_ok=true kind=(\w+)',s)
 valid=valid and len(nav)==2 and [(v[0],v[2],v[3],v[5]) for v in nav]==[('5','3','2','model'),('6','4','3','raster')]
 passed=bool(code==0 and valid and 'LIVE_MIXED_NAV_PASS' in s and 'cache_required=true' in s and 'tier=ram swap_reads_before=0 swap_reads_after=0 foreground=true' in s and 'LIVE_RECOVERY_FAIL' not in s and 'LIVE_RASTER_DECODE_REFUSED' not in s and hashlib.sha256(binary.read_bytes()).hexdigest()==identity)
 row={'format':ext,'source':source,'source_sha256':hashlib.sha256(target.read_bytes()).hexdigest(),'exit_code':code,'passed':passed,'neighbours':groups,'navigation':nav,'log_sha256':hashlib.sha256(log.read_bytes()).hexdigest()};r['cases'].append(row);r['status']='running' if passed else 'failed';(out/'result.json').write_text(json.dumps(r,indent=2)+'\n');print(ext,passed,code,flush=True)
 if not passed:raise SystemExit(1)
r['status']='passed';(out/'result.json').write_text(json.dumps(r,indent=2)+'\n')
