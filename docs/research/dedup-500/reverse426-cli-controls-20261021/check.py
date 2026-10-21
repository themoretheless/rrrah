from pathlib import Path
from PIL import Image
import tempfile, subprocess, json, re, hashlib
ROOT=Path(__file__).resolve().parents[4]
BIN=ROOT/'.dedup-work/reverse426-20261021/review-imgsearch'
rows=[]
with tempfile.TemporaryDirectory(prefix='reverse426-cli-') as tmp:
 p=Path(tmp); images=p/'images'; images.mkdir()
 source=images/'a-source.jpg'
 im=Image.new('RGB',(64,64)); im.putdata([(x*4,y*4,(x+y)*2) for y in range(64) for x in range(64)])
 im.save(source,quality=95); (images/'z-duplicate.jpg').write_bytes(source.read_bytes())
 corrupt=p/'corrupt'; corrupt.mkdir(); (corrupt/'bad.jpg').write_bytes(b'not a JPEG')
 png=p/'png'; png.mkdir(); im.save(png/'duplicate.png')
 for name, directory, extra, expected in [('top_one_self_exclusion',images,['-top','1'],0),('unlimited_positive',images,[],1),('missing_directory',p/'missing',[],0),('corrupt_candidate',corrupt,[],0),('png_discovery',png,[],0)]:
  args=[str(BIN),'-source',str(source),'-dir',str(directory),'-workers','1','-threshold','0']+extra
  r=subprocess.run(args,text=True,capture_output=True,timeout=30)
  match=re.search(r'Total matches found: (\d+)',r.stdout)
  count=int(match[1]) if match else None
  assert r.returncode==0 and count==expected,(name,r.returncode,r.stdout,r.stderr)
  rows.append(dict(name=name,exit_code=r.returncode,matches=count,stdout=r.stdout.replace(tmp,'<fixture>'),stderr=r.stderr.replace(tmp,'<fixture>')))
result=dict(repository='dpippenger/reverse-imagesearch',commit='617c17b544ff389d1de79a797e3ab05af3407bfe',binary_sha256=hashlib.sha256(BIN.read_bytes()).hexdigest(),original_cli=True,synthetic_fixtures=True,controls=rows,limits=['Observed behavior controls; passing assertions do not mean desired behavior.','PNG is excluded by discovery policy; no claim that its decoder fails.','Missing root behavior is explicitly expected by original FindImages test.'])
Path(__file__).with_name('result.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps([{'name':r['name'],'exit_code':r['exit_code'],'matches':r['matches']} for r in rows]))
