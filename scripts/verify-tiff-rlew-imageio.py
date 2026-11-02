"""macOS ImageIO full-image reference check; multi-row discrepancies stay explicit."""
from pathlib import Path
import hashlib,json,platform,subprocess,tempfile
from PIL import Image
root=Path(__file__).resolve().parents[1]/'tests/fixtures/raster'
results=[]
with tempfile.TemporaryDirectory(prefix='rrrah-rlew-imageio-') as temporary:
 for manifest in ['fax2w-single-libtiff-reference.json','fax2w-libtiff-reference.json','fax2w-long-libtiff-reference.json']:
  rows=json.loads((root/manifest).read_text())['rows'];exact=0;different=0;refused=0;examples=[]
  for row in rows:
   source=root/row['name'];assert hashlib.sha256(source.read_bytes()).hexdigest()==row['source_sha256']
   expected=(root/row['rgba8_file']).read_bytes();assert hashlib.sha256(expected).hexdigest()==row['rgba8_sha256']
   target=Path(temporary)/(source.stem+'.png')
   run=subprocess.run(['/usr/bin/sips','-s','format','png',str(source),'--out',str(target)],capture_output=True,text=True)
   if run.returncode or not target.exists():
    refused+=1;examples.append({'source':source.name,'status':'refused','returncode':run.returncode});continue
   image=Image.open(target).convert('RGBA');actual=image.tobytes()
   if image.size==(row['width'],row['height']) and actual==expected:exact+=1
   else:
    different+=1
    if len(examples)<3:examples.append({'source':source.name,'status':'different','actual_size':image.size,'differing_bytes':sum(a!=b for a,b in zip(actual,expected))+abs(len(actual)-len(expected))})
  result={'manifest':manifest,'manifest_sha256':hashlib.sha256((root/manifest).read_bytes()).hexdigest(),'sources':len(rows),'exact':exact,'different':different,'refused':refused,'examples':examples}
  results.append(result)
report={'scope':'System ImageIO via sips; independent application invocation, underlying codec ancestry unproven. One-row strips required exact; multi-row results diagnostic, not oracle success. No rrrah output used to derive expected pixels.','macos':platform.mac_ver()[0],'sips':subprocess.check_output(['/usr/bin/sips','--version'],text=True).strip(),'results':results}
print(json.dumps(report,indent=2))
assert results[0]['exact']==results[0]['sources'] and results[0]['refused']==0 and results[0]['different']==0
