from pathlib import Path
import sys,json,hashlib,tomllib,math
root=Path.cwd();sys.path[:0]=[str(root/'.dedup-work/mediahash478-extra-deps-20261021'),'/Users/themoretheless/.codex/tmp/rrrah-mediahash-deps-20261013']
import av
from PIL import Image,ImageDraw
cache=root/'.dedup-work/mediahash478-20261021';frames=[];digest=hashlib.sha256();timestamps=[]
with av.open(cache/'__test__/data/sample.mp4') as container:
 stream=container.streams.video[0];metadata={'width':stream.width,'height':stream.height,'rate':str(stream.average_rate),'codec':stream.codec_context.name,'duration':container.duration}
 for n,frame in enumerate(container.decode(video=0)):
  img=frame.to_image().convert('RGB');digest.update(img.tobytes());img.thumbnail((160,90));frames.append(img);timestamps.append(float(frame.time))
for start in range(0,len(frames),36):
 selected=frames[start:start+36];sheet=Image.new('RGB',(960,108*math.ceil(len(selected)/6)),(25,25,25));draw=ImageDraw.Draw(sheet)
 for i,img in enumerate(selected):x=(i%6)*160;y=(i//6)*108;sheet.paste(img,(x,y));draw.text((x,y+90),f'{start+i}: {timestamps[start+i]:.3f}s',fill='white')
 sheet.save(cache/f'contact-{start//36}.png')
lock=tomllib.loads((cache/'pdm.lock').read_text());packages=lock['package'];inventory={'metadata':lock.get('metadata'),'packages':len(packages),'names':[p['name'] for p in packages],'file_records':sum(len(p.get('files',[])) for p in packages),'git_packages':[p for p in packages if 'git' in p]}
for p in packages:
 for f in p.get('files',[]):
  h=f.get('hash','');assert h.startswith('sha256:') and len(h)==71
report={'decoded_frames':len(frames),'frame_rgb_sha256_sequence':digest.hexdigest(),'first_time':timestamps[0],'last_time':timestamps[-1],'video':metadata,'contact_sheets':math.ceil(len(frames)/36),'lock':inventory}
(root/'docs/research/dedup-500/mediahash478-media-review-20261021/result.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
