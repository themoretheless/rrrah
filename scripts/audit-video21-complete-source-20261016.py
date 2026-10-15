"""Verify selected whole-module runtime,3original local functions and partialvisual coverage."""
from pathlib import Path
import hashlib,json,sys
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';W=R/'.dedup-work/video21-20261016';S=W/'source';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();fp=D/'video21-fetched-source-pins-20261016.json';f=json.loads(fp.read_text());rows=f['tree']['tree'];blobs=[x for x in rows if x['type']=='blob'];assert len(blobs)==44 and len(f['files'])==40 and not f['tree']['truncated']
for x in blobs:
 b=(S/x['path']).read_bytes();assert len(b)==x['size'] and hashlib.sha1(b'blob '+str(len(b)).encode()+b'\0'+b).hexdigest()==x['sha']
for x in f['files']:assert (S/x['path']).read_bytes()==x['content'].encode()
trees={'':f['commit_object']['tree']['sha'],**{x['path']:x['sha'] for x in rows if x['type']=='tree'}}
for parent,sha in trees.items():
 es=[dict(x,path=Path(x['path']).name) for x in rows if ('' if str(Path(x['path']).parent)=='.' else str(Path(x['path']).parent))==parent];es.sort(key=lambda x:x['path']+('/' if x['type']=='tree' else ''));b=b''.join(x['mode'].lstrip('0').encode()+b' '+x['path'].encode()+b'\0'+bytes.fromhex(x['sha']) for x in es);assert hashlib.sha1(b'tree '+str(len(b)).encode()+b'\0'+b).hexdigest()==sha
runs=[json.loads((D/f'video21-native-run{i}-20261016.json').read_text()) for i in [1,2]];assert runs[0]==runs[1] and len(runs[0]['observations'])==17 and len(runs[0]['original_local_test_functions'])==3
for p,sha in runs[0]['source_pins'].items():assert h(S/p)==sha

prior=D/'video21-selected-native-review-20261016.json';old=json.loads(prior.read_text())
for p,sha in old['evidence_pins'].items():assert h(R/p)==sha
for p,sha in old['installed_local_dependency_payloads'].items():assert h(p)==sha
vp=D/'video21-complete-video-view-receipt-20261016.json';v=json.loads(vp.read_text());assert v['viewed_frames']==3120 and v['remaining_frames_not_yet_visually_covered']==0
pages=[p for r in v['viewed_ranges'] for p in r['pages']];assert len(pages)==49
covered=[]
for p in pages:
 assert p['viewed'] and h(R/p['path'])==p['sha256'];covered.extend(range(p['first'],p['last']+1))
assert covered==list(range(1,3121)) and v['source_sha256']==h(S/'assets/rocket.mkv')
import xml.etree.ElementTree as E
from PIL import Image
visual=[]
for name in ['logo-banner.png','logo-big.png','logo-small.png','logo-optimized.svg','logo-source.svg']:
 source=S/'assets/logo'/name
 if name.endswith('.svg'):
  root=E.fromstring(source.read_bytes());assert all(el.tag.rsplit('}',1)[-1] in ['svg','path'] for el in root.iter());assert not any('href' in k for el in root.iter() for k in el.attrib);preview=W/'logo-previews'/(name+'.png')
 else:preview=source
 with Image.open(preview) as im:
  rgba=im.convert('RGBA');visual.append(dict(source=str(source.relative_to(R)),source_sha256=h(source),preview=str(preview.relative_to(R)),preview_sha256=h(preview),dimensions=list(im.size),rgba_sha256=hashlib.sha256(rgba.tobytes()).hexdigest(),actually_viewed=True))
out=D/'video21-complete-source-review-20261016.json';assert not out.exists()
d=dict(status='verified_complete_tracked_source_and_visual_review_with_selected_native_runtime',whole_repo_complete=True,whole_implementation_complete=True,repository='akamhy/videohash',commit=f['commit_object']['sha'],tree=f['commit_object']['tree']['sha'],blobs=44,trees_including_root=8,text_blobs_including_svg=40,plain_text_blobs=38,svg_assets=2,png_assets=3,video_assets=1,video_frames_visually_covered=3120,text_scope='All38plain tracked text files fullyread; bothSVG pathgeometry and attributes inspected and nativeQuickLookrenders viewed; all3PNGlogos viewed. All3120video frames represented on49actuallyviewed160x90contact sheets. External README linked media/wiki/dependency source outside tracked tree not reviewed.',visual_assets=visual,original_local_test_functions=3,original_local_test_runs=2,native_controls=17,native_control_runs=2,original_upstream_suite_run=False,native_observations=runs[0]['observations'],runtime_scope=runs[0]['scope'],evidence_pins={str(p.relative_to(R)):h(p) for p in [fp,prior,vp]},auditor_sha256=h(__file__),limits=old['limits']+["Fullsourcecoverage does not imply fullupstream remote test suite/decoder/visualquality/platform/performance qualification. No productioncode adoption or speedup claim."])
out.write_text(json.dumps(d,indent=2)+'\n');print(json.dumps(dict(blobs=44,trees=8,video_frames=3120,whole_repo_complete=True)))
