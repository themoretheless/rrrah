"""Verify pinned Cleanarr tree, native controls and all reviewed media frames."""
from pathlib import Path
import hashlib,json
from PIL import Image
R=Path(__file__).resolve().parents[1];W=R/'.dedup-work/cleanarr29-20261016';D=R/'docs/research/dedup-500'
h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def emit(p,d):
 assert not p.exists();p.write_text(json.dumps(d,indent=2)+'\n')
def obj(kind,b):return hashlib.sha1(kind.encode()+b' '+str(len(b)).encode()+b'\0'+b).hexdigest()
g=json.loads((W/'github-tree.json').read_text());entries=g['tree']['tree'];assert not g['tree']['truncated'];blobs=[]
for e in entries:
 if e['type']!='blob':continue
 p=W/'source'/e['path'];b=p.read_bytes();assert len(b)==e['size'] and obj('blob',b)==e['sha']
 row={**e,'sha256':h(p)}
 try:row['text_content']=b.decode('utf8')
 except UnicodeDecodeError:row['binary']=True
 blobs.append(row)
assert len(blobs)==54 and sum('text_content' in x for x in blobs)==51
expected={'':g['commit']['tree']['sha'],**{e['path']:e['sha'] for e in entries if e['type']=='tree'}}
for directory,want in expected.items():
 children=[e for e in entries if str(Path(e['path']).parent).replace('.','',1)==directory] if directory=='' else [e for e in entries if str(Path(e['path']).parent)==directory]
 children.sort(key=lambda e:(Path(e['path']).name+('/' if e['type']=='tree' else '')).encode())
 b=b''.join(('40000' if e['type']=='tree' else e['mode']).encode()+b' '+Path(e['path']).name.encode()+b'\0'+bytes.fromhex(e['sha']) for e in children)
 assert obj('tree',b)==want,(directory,want,obj('tree',b))
assert len(expected)==12
controls=[]
for name,count in [('backend',19),('frontend',13)]:
 p=D/f'cleanarr29-{name}-controls-20261016.json';d=json.loads(p.read_text());assert len(d['runs'])==2 and d['runs'][0]==d['runs'][1] and len(d['runs'][0])==count
 producer=R/'scripts'/('qualify-cleanarr29-'+name+'-controls-20261016.'+('py' if name=='backend' else 'cjs'));assert h(producer)==d['producer_sha256']
 for key in ['source_pins','loaded_dependency_file_pins','tool_pins']:
  for path,want in d.get(key,{}).items():assert h(R/path)==want,path
 controls.append({'path':str(p.relative_to(R)),'sha256':h(p),'scenarios':count,'runs':2,'scope':d['scope'],'limits':d['limits']})
f=json.loads((W/'media-review/frames.json').read_text());im=Image.open(W/'source/screenshots/demo.gif');assert im.n_frames==85 and len(f['frames'])==85
for i,row in enumerate(f['frames']):
 im.seek(i);assert row['index']==i and hashlib.sha256(im.convert('RGBA').tobytes()).hexdigest()==row['rgba_sha256'] and im.info['duration']==row['duration_ms']
ico=Image.open(W/'source/frontend/public/favicon.ico');assert ico.ico.sizes()=={(16,16),(32,32),(48,48)}
for size in ico.ico.sizes():ico.ico.getimage(size).load()
Image.open(W/'source/screenshots/listing.png').load()
media={str(p.relative_to(R)):h(p) for p in (W/'media-review').glob('*') if p.is_file()}
assert len(list((W/'media-review').glob('gif-sheet-*.png')))==6
lock=json.loads((W/'lock-structured-review.json').read_text());parsed=json.loads((W/'parsed-yarn-lock.json').read_text());assert len(lock['rows'])==1470
acq=D/'cleanarr29-fetched-source-pins-20261016.json'
emit(acq,{'repository':'se1exin/Cleanarr','commit':g['commit']['sha'],'root_tree':expected[''],'canonical_trees_verified':expected,'blobs':blobs,'media_review_pins':media,'lock_metadata_review':lock,'limits':['51 UTF8 files including structured Yarn lock metadata; 3 binary assets reviewed visually. Dependency source trees not reviewed.']})
report={'status':'verified_complete_source_media_review_and_selected_native_controls','whole_repo_complete':True,'whole_implementation_complete':True,'repository':'se1exin/Cleanarr','commit':g['commit']['sha'],'blobs':54,'canonical_trees':12,'acquisition':str(acq.relative_to(R)),'acquisition_sha256':h(acq),'auditor_sha256':h(Path(__file__)),'controls':controls,'media_scope':'All85 GIF frames viewed on6contact sheets; native-resolution frames0/42/84 and listingPNG viewed; all3ICO sizes viewed. Published GIF depicts older UI, not current UI runtime. Frame pixels independently rehashed.','lock_scope':'All1470 unique resolution/version records and4012dependency references structurally reviewed; full inventory projection read in batches. No dependency-source or security audit.','license':'MIT LICENSE.md fully read; no production code copied','upstream_tests_run':False,'full_service_or_browser_run':False,'findings':['Native backend: sample TYPE predicate always excludes movies and episodes; show dispatch uses movie serializer; nonexistent media deletion reports success.','Native Flask testclient reads owned sibling through parent path; proxy forwards supplied URL to fake HTTP transport. No deployed server or external data access proof.','Native state modules: later-page errors leave loading true and an unhandled rejection; stale requests overwrite newer state; size request outer promise remains pending.','Native helpers: empty parts reduce throws; petabyte unit becomes undefined. Positive controls preserve selected media on delete error and preserve both TinyDB library fields.'],'decision':'Reject as portable comparer; retain review lessons for explicit deletion outcomes, cancellation/request generations, pagination failures and size boundaries. No adoption or speed claim.','limits':['External Plex/media objects and HTTP API transports replaced with owned fixtures; no real Plex, credentials, user media, deletion or network operations.','Complete pinned repository reading does not qualify dependencies, full React UI, upstream suites, platforms, permissions, visual accuracy or performance.']}
emit(D/'cleanarr29-complete-source-review-20261016.json',report)
print(json.dumps({'blobs':54,'trees':12,'backend_controls':38,'frontend_controls':26,'gif_frames':85,'status':'verified'}))
