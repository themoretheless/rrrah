"""Compare convex-window count fast path with exhaustive geometric taps."""
import hashlib,importlib.util,json
from pathlib import Path
module_path=Path('scripts/verify-dedup-filtered-rank-real.py')
spec=importlib.util.spec_from_file_location('filtered_rank_auditor',module_path);module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
models=[[[1.,0.,0.],[0.,1.,0.],[0.,0.,1.]],[[1.,0.,-3.],[0.,1.,5.],[0.,0.,1.]],[[.75,0.,1.],[0.,1.25,-2.],[0.,0.,1.]],[[0.,-1.,63.],[1.,0.,0.],[0.,0.,1.]],[[1.,.1,-3.],[-.1,1.,4.],[.002,-.001,1.]],[[2.,0.,0.],[0.,.5,0.],[.001,.003,1.]]]
coordinates=[-1,0,1,7,16,31,62,63,64,65];count=complete=0
for model in models:
 for x in coordinates:
  for y in coordinates:
   for radius in [0,3,8]:
    fast=module.window(model,x,y,radius,(64,64),(64,64),True);slow=module.window(model,x,y,radius,(64,64),(64,64),False)
    assert fast==slow,(model,x,y,radius,fast,slow);count+=1;complete+=fast[0]
assert count==1800 and complete>0
output=Path('docs/research/dedup-filtered-rank-window-oracle-controls.json');assert not output.exists();paths=[Path(__file__),module_path,module.helper,Path('scripts/dedup_jpeg_domain.py')]
output.write_text(json.dumps({'status':'passed','cases':count,'complete_windows':complete,'input_hashes':{str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in paths},'scope':'Six authored affine/projective mappings, edge/interior centers, filter0/3/8: geometric window validity and exact read counts match exhaustive taps in all1800 cases. Not luminance/rank counts or proof for all projective numerical inputs.'},indent=2)+'\n');print(count,'controls passed')
