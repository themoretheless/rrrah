"""Actual unchanged full module with modern native SIFT/FLANN, owned image fixtures."""
from pathlib import Path
import hashlib,json,sys,os,tempfile,unittest,importlib.util,importlib.metadata as metadata,contextlib,io
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';W=R/'.dedup-work/images62-20261015';S=W/'source';sys.path[:0]=[str(W/'deps'),str(S)];sys.dont_write_bytecode=True
import cv2 as cv,numpy as np
from PIL import Image
cv.setNumThreads(1);cv.setRNGSeed(123)
import main as original
os.chdir(S)
spec=importlib.util.spec_from_file_location('images62_original_tests',S/'tests.py');tests=importlib.util.module_from_spec(spec);spec.loader.exec_module(tests);suite=unittest.defaultTestLoader.loadTestsFromModule(tests);assert suite.countTestCases()==2;result=unittest.TextTestRunner(verbosity=2).run(suite);assert result.wasSuccessful()
cases=[];observations={}
def check(name,fn):observations[name]=fn();cases.append(name);print('PASS '+name)
def throws(kind,fn):
 try:fn()
 except kind as e:return type(e).__name__
 raise AssertionError('Expected '+kind.__name__)
def pipeline(directory):return original.similarity_check(original.detect_features(original.collect_imgs(str(directory))))
with tempfile.TemporaryDirectory(prefix='owned-native-',dir=W) as tmp:
 F=Path(tmp);noise=np.random.default_rng(918).integers(0,256,(512,512),dtype=np.uint8)
 def directory(name):p=F/name;p.mkdir();return p
 def save_pair(name,first,second):
  d=directory(name);Image.fromarray(first).save(d/'a.png');Image.fromarray(second).save(d/'b.png');return d
 def cli_options():
  before=(original.MIN_MATCHES,original.FEATURES_DISTANCE);argv=sys.argv;sys.argv=['main.py',str(F),'--min_matches','100000','--features_distance','0.99']
  try:args=original.argparser()
  finally:sys.argv=argv
  assert args.min_matches==100000 and args.features_distance==0.99 and (original.MIN_MATCHES,original.FEATURES_DISTANCE)==before;return {'parsed':[args.min_matches,args.features_distance],'actual_globals':list(before)}
 check('CLI_options_do_not_update_matching_globals',cli_options)
 def blank():
  d=save_pair('blank',np.zeros((128,128),np.uint8),np.zeros((128,128),np.uint8));imgs=original.detect_features(original.collect_imgs(str(d)));assert all(x['des'] is None for x in imgs);assert original.similarity_check(imgs)==[];return 'Twoidenticaldecodedpixelimages, noSIFTfeatures/no duplicate reported'
 check('native_featureless_identical_images_are_missed',blank)
 def invalid():
  d=directory('invalid');(d/'broken.PNG').write_bytes(b'not an image');imgs=original.collect_imgs(str(d));assert len(imgs)==1 and imgs[0]['f'] is None;return throws(cv.error,lambda:original.detect_features(imgs))
 check('unreadable_image_admitted_then_SIFT_raises',invalid)
 def threshold():
  results={}
  for n in [50,51]:
   desc=np.random.default_rng(n).random((n,128),dtype=np.float32);imgs=[{'p':f'{i}','f':noise,'des':desc.copy()} for i in [0,1]];results[str(n)]=original.similarity_check(imgs)
  assert results['50']==[] and results['51']==['0'];return {'output':results,'adapter':'Explicitdescriptorfixture, actualnativeFLANN andunchangedthresholdlogic'}
 check('strict_match_threshold_requires51_not50',threshold)
 def one_train():
  return throws(cv.error,lambda:original.similarity_check([{'p':'a','f':noise,'des':np.zeros((2,128),np.float32)},{'p':'b','f':noise,'des':np.ones((1,128),np.float32)}]))
 check('single_train_descriptor_cannot_satisfy_native_k2',one_train)
 def direction():
  many=np.zeros((51,128),np.float32);few=np.stack([np.zeros(128,np.float32),np.ones(128,np.float32)*100]);a={'p':'many','f':noise,'des':many};b={'p':'few','f':noise,'des':few};forward=original.similarity_check([a,b]);reverse=original.similarity_check([b,a]);assert forward==['many'] and reverse==[];return {'forward':forward,'reverse':reverse,'scope':'Syntheticdescriptorstate; native manytoone/asymmetricpolicy, not imagegroundtruth'}
 check('many_to_one_descriptor_matches_and_input_order_change_acceptance',direction)
 def duplicate_deletes():
  d=directory('triple');paths=[]
  for name in ['a','b','c']:p=d/f'{name}.png';Image.fromarray(noise).save(p);paths.append(str(p))
  records=original.detect_features(original.collect_imgs(str(d)));assert min(len(x['kp']) for x in records)>50;dups=original.similarity_check(records);assert len(dups)==3 and dups[0]==dups[1] and dups[2]!=dups[0];error=throws(FileNotFoundError,lambda:original.delete(dups));assert not Path(dups[0]).exists() and sum(Path(p).exists() for p in paths)==2;return {'duplicate_count':3,'unique_delete_paths':2,'remaining_files':2,'exception':error}
 check('three_real_identical_images_repeat_delete_path_and_abort',duplicate_deletes)
 def alpha():
  rgb=np.repeat(noise[:,:,None],3,axis=2);opaque=np.dstack([rgb,np.full(noise.shape,255,np.uint8)]);transparent=np.dstack([rgb,np.zeros(noise.shape,np.uint8)]);d=save_pair('alpha',opaque,transparent);records=original.collect_imgs(str(d));assert np.array_equal(records[0]['f'],records[1]['f']);dups=original.similarity_check(original.detect_features(records));assert len(dups)==1;assert not np.array_equal(opaque[:,:,:3],transparent[:,:,:3]*0);return {'different_alpha':True,'grayscale_equal':True,'duplicate_reported':True}
 check('opaque_vs_transparent_png_is_marked_duplicate',alpha)
 def animation():
  d=directory('animation');first=Image.fromarray(noise);seconds=[Image.fromarray(np.zeros(noise.shape,np.uint8)),Image.fromarray(np.full(noise.shape,255,np.uint8))]
  for name,second in zip(['a','b'],seconds):first.save(d/f'{name}.gif',save_all=True,append_images=[second],duration=[100,100],loop=0)
  frames=[]
  for name in ['a','b']:
   with Image.open(d/f'{name}.gif') as image:assert image.n_frames==2;image.seek(1);frames.append(np.asarray(image.convert('RGB')))
  assert not np.array_equal(frames[0],frames[1]);records=original.collect_imgs(str(d));assert all(x['f'] is not None for x in records) and np.array_equal(records[0]['f'],records[1]['f']);assert len(original.similarity_check(original.detect_features(records)))==1;return 'ActualnativeGIFframe0decode; secondframesblackvswhite ignored'
 check('different_second_GIF_frames_are_ignored_and_pair_marked_duplicate',animation)
 def mutation():
  d=save_pair('mutation',noise,noise);dups=pipeline(d);assert len(dups)==1;victim=Path(dups[0]);Image.fromarray(np.full(noise.shape,87,np.uint8)).save(victim);new_bytes=victim.read_bytes();assert new_bytes!=(d/('b.png' if victim.name=='a.png' else 'a.png')).read_bytes();original.delete(dups);assert not victim.exists();return 'Explicitpostmatchrewrite removed byactualdelete; newlyownedimageonly'
 check('actual_delete_removes_post_match_unique_rewrite',mutation)
 def missing():
  d=directory('missing-delete');a=d/'absent.png';b=d/'healthy.png';b.write_bytes(b'owned');error=throws(FileNotFoundError,lambda:original.delete([str(a),str(b)]));assert b.exists();return error
 check('missing_delete_path_aborts_before_later_entries',missing)
 def no_recursion():
  d=directory('nested');sub=d/'sub';sub.mkdir();Image.fromarray(noise).save(sub/'nested.png');assert original.collect_imgs(str(d))==[];return 'Originalnonrecursivepathpolicy, not a claimedbug'
 check('collection_is_nonrecursive',no_recursion)
 def colour():
  rgb=np.repeat(noise[:,:,None],3,axis=2);a=rgb.copy();b=rgb.copy();a[0,0]=[255,0,0];b[0,0]=[0,130,0];d=save_pair('colour',a,b);assert (d/'a.png').read_bytes()!=(d/'b.png').read_bytes();assert len(pipeline(d))==1;return 'DifferentRGBpixels stillnearimage match; candidate similarity doesnotprove byte/pixelidentity'
 check('near_match_is_not_exact_pixel_identity',colour)
 pins={str(p.relative_to(S)):hashlib.sha256(p.read_bytes()).hexdigest() for p in S.rglob('*') if p.is_file()}
 print(json.dumps({'status':'passed','original_tests':2,'controls':len(cases),'cases':cases,'observations':observations,'source_pins':pins,'versions':{x:metadata.version(x) for x in ['opencv-python-headless','numpy','Pillow']},'opencv':cv.__version__,'opencv_threads':cv.getNumThreads(),'python':sys.version,'adapters':['Entireunchangedmainmodule imported; entire2originaltestbodies unchanged, cwdoriginalsource.','Modernopencv-python-headless4.12/numpy2.2.6 actualSIFT/FLANN/decoder backend; declared2019opencv4.4twoGUIwheels notinstalled. Explicitheadlessmodernenvironment, no sourcepatch.','NativeRNGseed123/OpenCVthreads1 forreproducibility; explicitdescriptorfixtures onlythreshold/direction/k2 controls. OthercontrolsactualSIFT onownedPNG/GIF/RGBAorupstreamJPEG.','Deletiononlynewlyownedfixtures; explicitpostmatchrewrite, nooriginal/publicimage/source/userdatadeletion.'],'limits':['No calibratedlargepositive/negative visualquality/platform/original2019environment/performance qualification.','READMEexternalbranding/papers/badges notusedasruntimeevidence. No downloadedpaper/GUInativeclaim.','No Rootproductionadoption ornewlibraryspeedup.']}))
