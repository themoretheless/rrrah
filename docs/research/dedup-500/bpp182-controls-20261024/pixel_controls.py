"""Original full phash module, actual OpenCV/NumPy/Pillow; logger module shim.

Original is_transient and errno set extracted from retry.py into namespace shim.
Five original top-level test functions executed; four global-cluster test methods
previously qualified separately. No full application package graph or HEIC plugin.
"""
import ast,hashlib,importlib.util,json,logging,sys,tempfile,types
from pathlib import Path
import cv2,numpy as np
from PIL import Image
b=Path(__file__).resolve().parent;e=b.parent/'evidence/Arkalogy--best-photo-picker'
logshim=types.ModuleType('bpp.utils.logging');logshim.get_logger=logging.getLogger;sys.modules[logshim.__name__]=logshim
retry=types.ModuleType('bpp.utils.retry');tree=ast.parse((e/'remaining-pinned/bpp/utils/retry.py').read_text());nodes=[x for x in tree.body if isinstance(x,ast.FunctionDef) and x.name=='is_transient' or isinstance(x,ast.Assign) and any(isinstance(a,ast.Name) and a.id=='_TRANSIENT_ERRNOS' for a in x.targets)];exec(compile(ast.fix_missing_locations(ast.Module(body=nodes,type_ignores=[])),'original_retry_selection','exec'),retry.__dict__);sys.modules[retry.__name__]=retry
spec=importlib.util.spec_from_file_location('bpp.dedupe.phash',e/'05-phash.py');module=importlib.util.module_from_spec(spec);sys.modules[spec.name]=module;spec.loader.exec_module(module)
spec=importlib.util.spec_from_file_location('original_bpp_hash_tests',e/'06-test_dedupe_phash.py');tests=importlib.util.module_from_spec(spec);spec.loader.exec_module(tests);names=sorted(n for n in vars(tests) if n.startswith('test_'));testresults=[]
for name in names:
 try:getattr(tests,name)();testresults.append({'name':name,'passed':True})
 except Exception as ex:testresults.append({'name':name,'passed':False,'error':repr(ex)})
obs=[]
a=np.tile(np.arange(9,dtype=np.uint8)*28,(8,1));z=np.fliplr(a).copy();ha,hz=module.compute_dhash(a),module.compute_dhash(z);observed=module.hamming_distance(ha,hz);ref=((ha^hz)&((1<<64)-1)).bit_count();assert (ha,hz,observed,ref)==(-1,0,1,64);obs.append({'case':'opposite_gradient_pixels','hashes':[ha,hz],'original_distance':observed,'fixed_width64_distance':ref})
with tempfile.TemporaryDirectory(prefix='bpp182-pixels-') as root:
 root=Path(root);pa=root/'ascending.png';pz=root/'descending.png';Image.fromarray(a).save(pa);Image.fromarray(z).save(pz);got=[module.compute_hashes_from_file(str(p)) for p in [pa,pz]];assert [x[0] for x in got]==[-1,0];obs.append({'case':'original_png_loading_preserves_signed_hash_counterexample','hashes':got,'original_dhash_distance':module.hamming_distance(got[0][0],got[1][0])})
 pblack=root/'black.png';pwhite=root/'white.png';Image.fromarray(np.zeros((8,9),np.uint8)).save(pblack);Image.fromarray(np.full((8,9),255,np.uint8)).save(pwhite);hs=[module.compute_hashes_from_file(str(p)) for p in [pblack,pwhite]];assert hs==[(0,0),(0,0)];assert pblack.read_bytes()!=pwhite.read_bytes();obs.append({'case':'distinct_constant_black_white_collide_in_both_hashes','hashes':hs,'encoded_bytes_equal':False,'qualification':'Perceptual collision,not byte/pixel equality.'})
 bad=root/'invalid.png';bad.write_bytes(b'not an image');v=module.compute_hashes_from_file(str(bad));assert v==(None,None);obs.append({'case':'invalid_image_returns_unknown_hashes','result':v})
with tempfile.TemporaryDirectory(prefix='bpp182-orientation-') as root:
 root=Path(root); oriented=root/'oriented.jpg';rgb=np.zeros((24,40,3),np.uint8);rgb[:12,:20]=[255,0,0];rgb[12:,20:]=[0,255,0];im=Image.fromarray(rgb);exif=im.getexif();exif[274]=6;im.save(oriented,exif=exif)
 raw=cv2.imread(str(oriented),cv2.IMREAD_GRAYSCALE|cv2.IMREAD_IGNORE_ORIENTATION);expected=cv2.rotate(raw,cv2.ROTATE_90_CLOCKWISE);got=module._load_image(str(oriented));assert np.array_equal(got,expected);obs.append({'case':'jpeg_exif6_orientation_matches_independent_explicit_rotation','shape':list(got.shape)})
 pa=root/'transparent-ascending.png';pz=root/'transparent-descending.png';ra=np.dstack([a,a,a,np.zeros_like(a)]);rz=np.dstack([z,z,z,np.zeros_like(z)]);Image.fromarray(ra).save(pa);Image.fromarray(rz).save(pz);hs=[module.compute_hashes_from_file(str(p)) for p in [pa,pz]];assert [x[0] for x in hs]==[-1,0];assert np.all(ra[:,:,3]==0) and np.all(rz[:,:,3]==0);obs.append({'case':'invisible_rgb_changes_hashes','fully_transparent':True,'hashes':hs,'qualification':'Original grayscale loader ignores alpha; equal visible composition does not imply equal fingerprints at radius0.'})
p={'scope':__doc__,'original_top_level_test_results':testresults,'original_top_level_tests_passed':sum(x['passed'] for x in testresults),'authored_controls_passed':len(obs),'observations':obs,'versions':{'python':sys.version,'opencv':cv2.__version__,'numpy':np.__version__,'pillow':Image.__version__},'source_sha256':hashlib.sha256((e/'05-phash.py').read_bytes()).hexdigest(),'limits':'Finite synthetic arrays/owned local PNG; no real corpus accuracy, HEIC, EXIF orientation qualification, whole application/CLI or performance.'};(b/'pixel-results.json').write_text(json.dumps(p,indent=2)+'\n');print(json.dumps(p,indent=2));assert all(x['passed'] for x in testresults)
