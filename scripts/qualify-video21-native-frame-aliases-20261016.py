"""Stream all3120 native-resolution frames; alias only after complete decoded-byte equality."""
from pathlib import Path
from PIL import Image
import hashlib,json,subprocess
R=Path(__file__).resolve().parents[1];W=R/'.dedup-work/video21-20261016';asset=W/'source/assets/rocket.mkv';probe=W/'rocket-probe.json';meta=json.loads(probe.read_text());stream=meta['streams'][0];width,height=stream['width'],stream['height'];count=int(stream['nb_read_frames']);assert (width,height,count)==(640,360,3120);U=W/'native-unique-frames';U.mkdir(exist_ok=False);h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();seen={};rows=[];aliases=0;cmd=['ffmpeg','-v','error','-threads','1','-i',str(asset),'-fps_mode','passthrough','-pix_fmt','rgb24','-threads','1','-f','rawvideo','-'];frame_bytes=width*height*3;err=W/'native-frame-stream.log'
with err.open('x') as log:
 process=subprocess.Popen(cmd,stdout=subprocess.PIPE,stderr=log)
 try:
  for i in range(1,count+1):
   raw=process.stdout.read(frame_bytes);assert len(raw)==frame_bytes,(i,len(raw));sha=hashlib.sha256(raw).hexdigest()
   if sha in seen:
    first,path=seen[sha];im=Image.open(path).convert('RGB');assert im.tobytes()==raw,'Digestcollision must not collapse distinctframe bytes';im.close();aliases+=1
   else:
    path=U/f'frame-{i:05}.png';Image.frombytes('RGB',(width,height),raw).save(path);seen[sha]=(i,path);first=i
   rows.append({'frame':i,'native_rgb_sha256':sha,'representative_frame':first})
  assert not process.stdout.read(1) and process.wait(timeout=30)==0
 except BaseException:
  if process.poll() is None:process.terminate();process.wait(timeout=10)
  raise
out=W/'native-frame-aliases.json';out.write_text(json.dumps({'status':'verified_full_native_frame_byte_aliases','source_sha256':h(asset),'probe_sha256':h(probe),'command':cmd,'width':width,'height':height,'frames':count,'unique_frames':len(seen),'byte_identical_aliases':aliases,'representatives':[{'frame':i,'path':str(path),'png_sha256':h(path),'native_rgb_sha256':sha} for sha,(i,path) in seen.items()],'rows':rows,'producer_sha256':h(__file__),'scope':'Full640x360RGB frames streamed; every repeatedSHA bucket confirmed byentiredecodedRGB bytecomparison withlosslessrepresentativePNG. No perceptualhash/thumbnaildedup. Visualreview ofrepresentatives stillrequired; no framecodec/fullhashquality/performance/adoption proof.'},indent=2)+'\n');print(json.dumps({'frames':count,'unique_native_rgb_frames':len(seen),'byte_confirmed_aliases':aliases}))
