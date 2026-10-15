"""Verify all tracked source/tree/assets and twice-run selected native control evidence."""
from pathlib import Path
import ast,hashlib,json,xml.etree.ElementTree as E
from PIL import Image
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';W=R/'.dedup-work/plex24-20261016';S=W/'source';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
fpath=D/'plex24-fetched-source-pins-20261016.json';f=json.loads(fpath.read_text());rows=f['tree']['tree'];blobs=[x for x in rows if x['type']=='blob'];assert len(blobs)==17 and len(f['files'])==16 and not f['tree']['truncated']
for x in blobs:
 b=(S/x['path']).read_bytes();assert len(b)==x['size'] and hashlib.sha1(b'blob '+str(len(b)).encode()+b'\0'+b).hexdigest()==x['sha']
for x in f['files']:assert (S/x['path']).read_bytes()==x['content'].encode()
trees={'':f['commit_object']['tree']['sha'],**{x['path']:x['sha'] for x in rows if x['type']=='tree'}}
for parent,sha in trees.items():
 es=[dict(x,path=Path(x['path']).name) for x in rows if ('' if str(Path(x['path']).parent)=='.' else str(Path(x['path']).parent))==parent];es.sort(key=lambda x:x['path']+('/' if x['type']=='tree' else ''));b=b''.join(x['mode'].lstrip('0').encode()+b' '+x['path'].encode()+b'\0'+bytes.fromhex(x['sha']) for x in es);assert hashlib.sha1(b'tree '+str(len(b)).encode()+b'\0'+b).hexdigest()==sha
runs=[json.loads((D/f'plex24-selected-run{i}-20261016.json').read_text()) for i in [1,2]];assert runs[0]==runs[1] and len(runs[0]['controls'])==21
for p,sha in runs[0]['source_pins'].items():assert h(S/p)==sha
assert runs[0]['producer_sha256']==h(R/'scripts/qualify-plex24-selected-controls-20261016.py')
vpath=D/'plex24-visual-view-receipt-20261016.json';v=json.loads(vpath.read_text());assert len(v['frames'])==71 and len(v['pages'])==8 and v['source_sha256']==h(S/'assets/demo.gif')
with Image.open(S/'assets/demo.gif') as im:
 assert im.n_frames==71 and list(im.size)==v['dimensions']
 for i,row in enumerate(v['frames']):
  im.seek(i);rgb=im.convert('RGB');assert row['frame']==i+1 and hashlib.sha256(rgb.tobytes()).hexdigest()==row['native_rgb_sha256'] and im.info.get('duration')==row['duration_ms']
covered=[]
for p in v['pages']:assert p['viewed'] and h(R/p['path'])==p['sha256'];covered.extend(range(p['first'],p['last']+1))
assert covered==list(range(1,72))
for p in v['additional_native_views']:assert p['viewed'] and h(R/p['path'])==p['sha256']
root=E.fromstring((S/'assets/logo.svg').read_bytes());assert not any('href' in k for el in root.iter() for k in el.attrib)
assert v['logo']['viewed'] and h(R/v['logo']['path'])==v['logo']['sha256']
assert not any('test' in x['path'].lower() for x in blobs)
producer=R/'scripts/qualify-plex24-selected-controls-20261016.py';pins={str(p.relative_to(R)):h(p) for p in [fpath,vpath,producer,D/'plex24-selected-run1-20261016.json',D/'plex24-selected-run2-20261016.json']}
out=D/'plex24-complete-source-review-20261016.json';assert not out.exists();out.write_text(json.dumps(dict(status='verified_complete_tracked_source_and_visual_review_with_selected_native_controls',whole_repo_complete=True,whole_implementation_complete=True,repository='l3uddz/plex_dupefinder',commit=f['commit_object']['sha'],tree=f['commit_object']['tree']['sha'],blobs=17,trees_including_root=len(trees),plain_text_blobs=15,svg_assets=1,gif_assets=1,visual_frames=71,scope='All15plaintrackedtext files fullyread including full GPLv3 andoriginalmain/config/shell bodies. SVG geometry/style inspected andnativepreview viewed. EveryGIFframe viewed on8uncropped640widecontact sheets, nativeframes36and71 additionallyviewed forlegibility. ExternalREADMElinked screenshots/asciinema andthirdpartydependency sources outside trackedtree notreviewed.',native_controls=21,native_runs=2,native_scope=runs[0]['scope'],native_observations=runs[0]['controls'],original_upstream_suite_run=False,original_tests_tracked=0,evidence_pins=pins,auditor_sha256=h(__file__),decision='Referenceonly; metadata-scoring andremoteAPIdeletion are not exact/visual proof. No upstreamcodeadopted.',preparation_corrections=['Connector contentsendpoint returned rawtext ratherthanJSON; initialJSONparse failed before save. Refetchedrawtext andverified allGitblobs.','Firstselectedmain harness copied namespace while functions retained originalglobals, raisingmissingplex; corrected harnesssharedglobals, then21controls twice identical. Upstreamsource unchanged.'],limits=['Wholeoriginalpackage notimported: selectedoriginalAST bodies/main execute with explicit syntheticPlex/HTTP/UI/time adapters. Not realPlexAPI/requests/tabulate qualification.','No HTTP requests, accounts, credentials, userfiles oractualdeletion used. Shellscript read butnot executed.','No speedup, performance, fullformat/visualquality, platform or productionadoption claim.']),indent=2)+'\n');print(json.dumps(dict(blobs=17,trees=len(trees),native_controls=21,runs=2,visual_frames=71,whole_repo_complete=True)))
