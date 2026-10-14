#!/usr/bin/env python3
"""Complete captured RetinaHash source and synthetic behavior controls; no upstream tests exist."""
from pathlib import Path
import hashlib,json,sys,shutil,tomllib,math,warnings
import numpy as np
from scipy.fft import dctn
R=Path(__file__).resolve().parents[1];D=R/'docs/research';E=D/'dedup-500/evidence/bryanbarcelona--retinahash';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
registry=D/'dedup-500/reviews/499.json';reg=json.loads(registry.read_text());tree=json.loads((E/'tree.json').read_text());assert not tree['truncated']
paths={r['path']:r['local'] for key in ['available_source_evidence','available_readme_license_evidence'] for r in reg[key]}
paths.update({'.github/workflows/ci.yml':'06-ci.yml','.github/workflows/release.yml':'07-release.yml','.gitignore':'08-gitignore','.python-version':'09-python-version','CHANGELOG.md':'10-CHANGELOG.md','pyproject.toml':'11-pyproject.toml','src/retinahash/core/utils.py':'12-utils.py','uv.lock':'13-uv.lock'})
blobs={r['path']:r for r in tree['tree'] if r['type']=='blob'};assert set(paths)==set(blobs) and len(blobs)==14
rows=[]
for name,local in paths.items():
 p=E/local;b=p.read_bytes();assert len(b)==blobs[name]['size'] and hashlib.sha1(b'blob '+str(len(b)).encode()+b'\0'+b).hexdigest()==blobs[name]['sha'];rows.append(dict(path=name,local=str(p.relative_to(R)),sha256=sha(p),git_blob_sha1=blobs[name]['sha'],bytes=len(b)))
F=Path('/private/tmp/rrrah-retinahash499-controls-20261014');F.mkdir(exist_ok=False)
for name,local in paths.items():
 if name.startswith('src/'):
  p=F/name;p.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(E/local,p)
sys.path.insert(0,str(F/'src'))
import retinahash
from retinahash import RetinaHash as H
from retinahash.core.transforms import resize,to_gray,autocrop,dct_2d
from retinahash.core.utils import bits_to_hex
checks=[]
def check(name,actual,expected):
 assert actual==expected,(name,actual,expected);checks.append(dict(name=name,actual=actual,expected=expected))
def raises(name,fn,kind):
 try:fn()
 except kind as e:checks.append(dict(name=name,error=type(e).__name__));return
 raise AssertionError(name)
rng=np.random.default_rng(499);im=rng.integers(0,256,(39,61,3),dtype=np.uint8)
h=H.phash(im);check('default64bits_hex_width',len(h),16)
for k in range(4):check(f'rotation_invariance_{k}',H.phash(np.rot90(im,k),rotation_invariant=True),H.phash(im,rotation_invariant=True))
a=np.concatenate([im,np.zeros((*im.shape[:2],1),dtype=np.uint8)],axis=2);b=a.copy();b[:,:,:3]=255-im
check('all_transparent_different_hidden_rgb_is_not_normalized',H.phash(a)!=H.phash(b),True)
opaque=a.copy();opaque[:,:,3]=255;check('alpha_changes_ignored',H.phash(a),H.phash(opaque))
check('normalized_autocrop_does_not_remove_white_border',autocrop(np.pad(np.zeros((3,4)),2,constant_values=1)).shape,(7,8))
check('uint8_white_border_removed',autocrop(np.pad(np.zeros((3,4)),2,constant_values=255)).shape,(3,4))
with warnings.catch_warnings():
 warnings.simplefilter('ignore');check('nan_image_collides_with_infinity_image',H.phash(np.full((8,8),np.nan)),H.phash(np.full((8,8),np.inf)));check('hash_size1_zero_bit',H.phash(im,hash_size=1),'0')
raises('hash_size0_late_value_error',lambda:H.phash(im,hash_size=0),ValueError)
check('oversized_hash_silently_clamped',H.phash(im,hash_size=33),H.phash(im,hash_size=32))
check('odd_hash_size_9bits_all_zero_missing_hex_padding',bits_to_hex(np.zeros((3,3),dtype=bool)),'00')
check('hamming_unequal_widths_accepted',H.distance('0','00000000'),0)
raises('single_channel_3d_input_late_dot_error',lambda:H.phash(np.zeros((8,8,1))),ValueError)
raises('empty_image_late_index_error',lambda:H.phash(np.zeros((0,8))),IndexError)
raises('invalid_method',lambda:H.hash(im,method='missing'),ValueError)
for n in [1,2,4,8,32]:
 q=rng.normal(size=(n,n));err=float(np.max(np.abs(dct_2d(q)-dctn(q,type=2,norm=None)/4)));assert err<1e-10;checks.append(dict(name=f'dct_independent_scipy_{n}',max_abs_error=err,tolerance=1e-10))
q=np.arange(6).reshape(2,3);got=resize(q,5,4);expected=np.empty((4,5))
for y in range(4):
 for x in range(5):
  sx=x*2/4;sy=y/3;ix=int(sx);iy=int(sy);fx=sx-ix;fy=sy-iy;jx=min(ix+1,2);jy=min(iy+1,1);expected[y,x]=(1-fx)*(1-fy)*q[iy,ix]+fx*(1-fy)*q[iy,jx]+(1-fx)*fy*q[jy,ix]+fx*fy*q[jy,jx]
assert np.allclose(got,expected,rtol=0,atol=1e-14);checks.append(dict(name='bilinear_independent_scalar_oracle',max_abs_error=float(np.max(np.abs(got-expected)))))
project=tomllib.loads((E/'11-pyproject.toml').read_text());lock=tomllib.loads((E/'13-uv.lock').read_text());check('console_entrypoint_missing',hasattr(retinahash,'main'),False);check('declared_console_target',project['project']['scripts']['retinahash'],'retinahash:main');check('numpy_runtime_exact_locked',np.__version__,next(p['version']for p in lock['package']if p['name']=='numpy'))
packages=[]
for p in lock['package']:
 artifacts=p.get('wheels',[])+([p['sdist']]if 'sdist'in p else [])
 for v in artifacts:assert v['hash'].startswith('sha256:') and len(v['hash'])==71 and v['size']>0
 packages.append(dict(name=p['name'],version=p['version'],artifacts=len(artifacts),dependencies=p.get('dependencies',[])))
check('no_upstream_test_files_in_tree',any('/test' in p or p.startswith('tests/')for p in paths),False)
O=D/'dedup-retinahash499-controls-20261014.json';assert not O.exists();O.write_text(json.dumps(dict(status='verified_complete_captured_source_and_synthetic_controls',repository=reg['repository'],commit=reg['commit'],captured_tree_root=tree['sha'],source_blobs=rows,checks=checks,checks_passed=len(checks),upstream_tests_run=False,lock_packages=packages,producer_sha256=sha(__file__),numpy_version=np.__version__,scope='All14captured Git tree blobs pinned and reviewed, original unmodified5Python modules imported. Synthetic NumPy controls and independent SciPy DCT/scalar bilinear oracles; no upstream tests present, no external image corpus, format/ICC/EXIF/resource/cancellation or whole-image identity qualification. Lock artifact metadata parsed and validated, not installed or remotely verified. Original commit-to-tree binding not independently fetched. No foreign code adopted.'),indent=2)+'\n');print(len(checks),'controls passed')
