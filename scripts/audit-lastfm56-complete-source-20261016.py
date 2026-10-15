"""Verify pinned tree, four native scripts, decoded PSD layers and embedded screenshot."""
from pathlib import Path
import hashlib,json,sys
R=Path(__file__).resolve().parents[1];W=R/'.dedup-work/lastfm56-20261016';D=R/'docs/research/dedup-500';sys.path.insert(0,str(W/'python-deps'))
from psd_tools import PSDImage
from PIL import Image
h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def emit(p,d):assert not p.exists();p.write_text(json.dumps(d,indent=2)+'\n')
def obj(k,b):return hashlib.sha1(k.encode()+b' '+str(len(b)).encode()+b'\0'+b).hexdigest()
g=json.loads((W/'github-commit.json').read_text());t=json.loads((D/'evidence/shevchenkoartem--lastfm-smart-deduper/tree.json').read_text());assert g['tree']['sha']==t['sha'] and not t['truncated'];entries=t['tree'];blobs=[]
for e in entries:
 if e['type']!='blob':continue
 p=W/'source'/e['path'];b=p.read_bytes();assert len(b)==e['size'] and obj('blob',b)==e['sha'];row={**e,'sha256':h(p)}
 if p.suffix!='.psd':row['text_content']=b.decode()
 blobs.append(row)
expected={'':t['sha'],**{e['path']:e['sha'] for e in entries if e['type']=='tree'}}
for dirname,want in expected.items():
 children=[e for e in entries if ('' if str(Path(e['path']).parent)=='.' else str(Path(e['path']).parent))==dirname];children.sort(key=lambda e:(Path(e['path']).name+('/' if e['type']=='tree' else '')).encode());b=b''.join(('40000' if e['type']=='tree' else e['mode']).encode()+b' '+Path(e['path']).name.encode()+b'\0'+bytes.fromhex(e['sha']) for e in children);assert obj('tree',b)==want
assert len(blobs)==9 and len(expected)==4
p=D/'lastfm56-native-controls-20261016.json';d=json.loads(p.read_text());assert h(R/'scripts/qualify-lastfm56-native-controls-20261016.cjs')==d['producer_sha256']
for key in ['source_pins','loaded_dependency_file_pins']:
 for name,want in d[key].items():assert h(R/name)==want
assert len(d['runs'])==2 and d['runs'][0]==d['runs'][1]
for run in d['runs']:
 assert len(run)==4 and all(len(v['scenarios'])==17 for v in run);assert run[0]['scenarios']==run[2]['scenarios'];assert all(not x['clicks'] for row in run[3]['scenarios'] for x in row['state'])
a=W/'media-review';meta=json.loads((a/'layers.json').read_text());psd=PSDImage.open(W/'source/other/media/lastfm-smart-deduper.psd');layers=list(psd.descendants());assert len(layers)==len(meta['layers'])==11 and psd.size==(1280,640)
for i,(layer,row) in enumerate(zip(layers,meta['layers'])):
 assert row['index']==i and row['name']==layer.name and row['bbox']==list(layer.bbox)
 if layer.has_pixels():assert hashlib.sha256(layer.topil().tobytes()).hexdigest()==row['pixel_sha256'];assert Image.open(a/f'layer-{i}.png').tobytes()==layer.topil().tobytes()
assert sum(l.has_pixels() for l in layers)==9
assert layers[7].smart_object.data==(a/'embedded-smart-object.bin').read_bytes();embedded=Image.open(a/'embedded-smart-object.bin');assert embedded.size==(1116,1304) and embedded.tobytes()==Image.open(a/'embedded-smart-object.png').tobytes()
assert psd.topil().tobytes()==Image.open(a/'composite.png').tobytes();Image.open(W/'readme-banner.png').load()
media={str(f.relative_to(R)):h(f) for f in a.iterdir() if f.is_file()};media[str((W/'readme-banner.png').relative_to(R))]=h(W/'readme-banner.png')
acq=D/'lastfm56-fetched-source-pins-20261016.json';emit(acq,{'repository':'shevchenkoartem/lastfm-smart-deduper','commit':g['sha'],'canonical_trees':expected,'blobs':blobs,'media_review_pins':media,'psd_layers':meta,'readme_image_url':'https://repository-images.githubusercontent.com/226846544/220b3880-1acd-11ea-9894-d310454be9ae'})
emit(D/'lastfm56-complete-source-review-20261016.json',{'status':'verified_complete_pinned_source_media_and136_native_script_executions','whole_repo_complete':True,'whole_implementation_complete':True,'repository':'shevchenkoartem/lastfm-smart-deduper','commit':g['sha'],'blobs':9,'canonical_trees':4,'acquisition':str(acq.relative_to(R)),'acquisition_sha256':h(acq),'auditor_sha256':h(Path(__file__)),'controls':str(p.relative_to(R)),'controls_sha256':h(p),'media_scope':'PSD stored mergedpreview1280x640 viewed; all9rasterlayer previews oncontact sheet,metadata11nodes incl2groups andtype text reviewed; embeddedPNG1116x1304 viewedfull; externalREADMEbanner viewed. All layer pixels/embeddedpayload rehashed. No Photoshop recomposition/effect/font-engine execution claim. No videos.','license':'Complete MIT LICENSE read; no production code copied.','upstream_tests_run':False,'native_scope':d['scope'],'findings':d['findings'],'decision':'Reject scrobble/title/time heuristic as exact/image identity. Retain tuple-key boundaries, uncertain-candidate/action-acknowledgment and explicit manual-choice lessons; no adoption orspeedclaim.','limits':d['limits']});print(json.dumps({'status':'verified','blobs':9,'trees':4,'script_executions':136,'PSD_raster_layers':9,'embeddedPNG':True}))
