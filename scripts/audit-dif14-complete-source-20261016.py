"""Verify complete58blob trackedsource/asset review and real declared-dependency controls."""
from pathlib import Path
import ast,base64,hashlib,io,json,xml.etree.ElementTree as E
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';W=R/'.dedup-work/dif14-20261016';S=W/'source';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();fp=D/'dif14-fetched-source-pins-20261016.json';f=json.loads(fp.read_text());rows=f['tree']['tree'];assert not f['tree']['truncated'];blobs=[x for x in rows if x['type']=='blob'];assert len(blobs)==58
for x in blobs:
 b=(S/x['path']).read_bytes();assert len(b)==x['size'] and hashlib.sha1(b'blob '+str(len(b)).encode()+b'\0'+b).hexdigest()==x['sha']
for x in f['files']:
 b=(S/x['path']).read_bytes()
 if 'content' in x:assert b==x['content'].encode()
 else:assert b.hex()==x['raw_hex'] and b.decode(x['encoding'])==x['decoded_text'] and x['decoded_text'].encode(x['encoding'])==b
assert len(f['files'])==35
trees={'':f['commit_object']['tree']['sha'],**{x['path']:x['sha'] for x in rows if x['type']=='tree'}}
for parent,sha in trees.items():
 es=[dict(x,path=Path(x['path']).name) for x in rows if ('' if str(Path(x['path']).parent)=='.' else str(Path(x['path']).parent))==parent];es.sort(key=lambda x:x['path']+('/' if x['type']=='tree' else ''));b=b''.join(x['mode'].lstrip('0').encode()+b' '+x['path'].encode()+b'\0'+bytes.fromhex(x['sha']) for x in es);assert hashlib.sha1(b'tree '+str(len(b)).encode()+b'\0'+b).hexdigest()==sha
runs=[json.loads((D/f'dif14-native-run{i}-20261016.json').read_text()) for i in [1,2]];assert runs[0]==runs[1] and len(runs[0]['observations'])==16 and runs[0]['dependencies']=={'numpy':'1.26.4','Pillow':'10.2.0'}
for p,sha in runs[0]['source_pins'].items():assert h(S/p)==sha
assert not any(isinstance(x,ast.FunctionDef) and x.name.startswith('test_') for x in ast.walk(ast.parse((S/'difPy/dif.py').read_text())))
vp=D/'dif14-visual-view-receipt-20261016.json';v=json.loads(vp.read_text());assert len(v['images'])==34 and len(v['pages'])==6
for x in v['pages']:assert x['viewed'] and h(R/x['path'])==x['sha256']
# Decode original assets afresh using the exact declared Pillow installation.
import sys
sys.path.insert(0,str(W/'deps'));from PIL import Image
assert Image.__version__=='10.2.0'
seen=set()
for x in v['images']:
 label=x['label']
 if ' embedded' in label:
  name,index=label.rsplit(' embedded',1);el=list(E.fromstring((S/name).read_bytes()).iter())[int(index)];uri=next(z for z in el.attrib.values() if z.startswith('data:image/png;base64,'));b=base64.b64decode(uri.split(',',1)[1],validate=True);assert hashlib.sha256(b).hexdigest()==x['input_sha256'];im=Image.open(io.BytesIO(b)).convert('RGBA');seen.add(name)
 elif '.ico ' in label:
  name,size=label.split('.ico ',1);name+='.ico';im=Image.open(S/name).ico.getimage(ast.literal_eval(size)).convert('RGBA');assert h(S/name)==x['input_sha256'];seen.add(name)
 elif label.endswith('.svg'):
  im=Image.open(W/(Path(label).name+'.png')).convert('RGBA');assert h(S/label)==x['input_sha256'];seen.add(label)
 else:im=Image.open(S/label).convert('RGBA');assert h(S/label)==x['input_sha256'];seen.add(label)
 assert list(im.size)==x['dimensions'] and hashlib.sha256(im.tobytes()).hexdigest()==x['rgba_pixels_sha256']
assert seen=={x['path'] for x in blobs if x['path'].endswith(('.png','.ico','.svg'))} and len(seen)==25
payloads={str(p):h(p) for p in (W/'deps').rglob('*') if p.is_file() and '__pycache__' not in p.parts and p.suffix!='.pyc'};assert payloads
pins={str(p.relative_to(R)):h(p) for p in [fp,vp,R/'scripts/qualify-dif14-native-controls-20261016.py']+list(D.glob('dif14-native-run*-20261016.json'))+list(D.glob('dif14-*-20261016.log'))}
out=D/'dif14-complete-source-review-20261016.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_complete_tracked_source_visual_review_and_native_controls','whole_repo_complete':True,'whole_implementation_complete':True,'repository':'elisemercury/Duplicate-Image-Finder','commit':f['commit_object']['sha'],'tree':f['commit_object']['tree']['sha'],'blobs':58,'trees_including_root':15,'plain_text_blobs':33,'svg_assets':2,'png_assets':22,'ico_assets':1,'actual_visual_views':34,'text_scope':'All33plain text blobs fullyread including2UTF16requirements recoveredvia rawbyte/GitSHA verification. BothSVG XMLgeometry/filter/mask/attribute trees fullyinspected,4embeddedPNG occurrences decodedandviewed, nativeQuickLookrenders viewed. Base64pixel payload interpretedthroughdecodedimages, not literalcharacterreading.','native_controls':16,'native_runs':2,'declared_dependencies':runs[0]['dependencies'],'native_execution_scope':runs[0]['scope'],'original_upstream_suite_run':False,'original_tests_tracked':0,'native_observations':runs[0]['observations'],'evidence_pins':pins,'installed_nonpyc_dependency_payloads':payloads,'auditor_sha256':h(__file__),'preparation_corrections':['Connector rejected2UTF16requirements; firstsave raisedKeyError beforecomplete. Recoveredrawbytes/UTF16roundtrip andverifiedall58Gitblobs.','Naive rglob textread reachedgenerated__pycache__ andstopped; resumedusingexacttrackedfilelist, notcachefiles.','QuickLook sandboxinitialization rejected beforepreview; native escalatedread-onlypreview succeeded.'],'limits':['Alltrackedsource/docs/assets reviewed, not independentNumPy/Pillow/dependency source audit. Exactdeclaredversions installedandactualruntimeused.','Two small completePool1build/searchscenarios andselectedfullmodule controls, no upstreamtest suitepresent; no5001imagefullpipeline/CLI/DesktopGUI/Windows/Linux/fullformat/quality/RSS/throughput qualification.','Desktop/screenshotsperformance figures are documentation only, no currentbenchmark claims.','Do not adopt batchsum-equality/uint8MSE/missingmaxID/partialrotation/mutationunsafeoperations; no upstreamproductioncode adopted andno speedup claim.']},indent=2)+'\n');print(json.dumps({'blobs':58,'trees':15,'asset_files':25,'visual_views':34,'controls':16,'runs':2,'dependency_payloads':len(payloads),'whole_repo_complete':True}))
