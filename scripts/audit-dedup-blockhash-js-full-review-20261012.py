#!/usr/bin/env python3
import hashlib,json
from fractions import Fraction as F
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def weighted(im,bits=4):
 w,h=im['width'],im['height'];block=[]
 for by in range(bits):
  for bx in range(bits):
   total=F(0)
   for y in range(h):
    dy=max(F(0),min(F(y+1),F((by+1)*h,bits))-max(F(y),F(by*h,bits)))
    for x in range(w):
     dx=max(F(0),min(F(x+1),F((bx+1)*w,bits))-max(F(x),F(bx*w,bits)));p=im['data'][(y*w+x)*4:(y*w+x+1)*4];v=765 if p[3]==0 else sum(p[:3]);total+=v*dx*dy
   block.append(total)
 output=[];n=bits*bits//4;half=F(w*h,bits*bits)*384
 for band in range(4):
  values=block[band*n:(band+1)*n];ordered=sorted(values);median=(ordered[(n-1)//2]+ordered[n//2])/2
  output.extend(int(v>median or(abs(v-median)<1 and median>half))for v in values)
 return ''.join(format(sum(output[i+j]<<(3-j)for j in range(4)),'x')for i in range(0,len(output),4))
def main():
 out=D/'blockhash-js-full-review-20261012.json';assert not out.exists();mp=D/'blockhash-js-full-source-20261012.json';m=json.loads(mp.read_text());s=Path(m['snapshot']);h=T/'rrrah-blockhash-js-runtime-20261012';assert all(sha(s/k)==sha(h/k)==v for k,v in m['files'].items())
 tree=D/'evidence/commonsmachinery--blockhash-js/tree.json';blobs={x['path']:x['sha']for x in json.loads(tree.read_text())['tree']if x['type']=='blob'};assert set(blobs)==set(m['files'])
 for k,v in blobs.items():
  b=(s/k).read_bytes();assert hashlib.sha1(b'blob '+str(len(b)).encode()+b'\0'+b).hexdigest()==v
 tests=T/'rrrah-blockhash-js-original-tests-20261012.json';stats=json.loads(tests.read_text())['stats'];assert stats['tests']==stats['passes']==34 and stats['failures']==stats['pending']==0
 controls=T/'rrrah-blockhash-js-edge-controls-20261012.json';c=json.loads(controls.read_text());assert all(weighted(im)==im['hash']for im in c['weighted_fixtures']);assert c['invalid_hex_distance']==0 and c['method1_edge_omission']['base']==c['method1_edge_omission']['changed'] and c['method2_edge_included']=={'base':'0000','changed':'000f'}
 paths=[mp,tree,tests,controls,T/'rrrah-blockhash-js-original-tests-20261012.stderr',T/'rrrah-blockhash-js-install-20261012.log',T/'rrrah-blockhash-js-test-dependencies-20261012.log',h/'deps/package.json',h/'deps/package-lock.json',R/'scripts/probe-dedup-blockhash-js-20261012.cjs',Path(__file__),Path('/Users/themoretheless/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/bin/node')]+[s/k for k in m['files']];deps={str(p.relative_to(h/'deps')):sha(p)for p in (h/'deps/node_modules').rglob('*')if p.is_file()};dependency_manifest=T/'rrrah-blockhash-js-dependency-pins-20261012.json';assert not dependency_manifest.exists();dependency_manifest.write_text(json.dumps(deps,sort_keys=True,indent=2)+'\n');paths.append(dependency_manifest)
 report={'status':'full_pinned_source_review_and_34_original_tests_verified','repository':m['repository'],'commit':m['commit'],'all_git_blobs_verified':True,'files':len(m['files']),'text_files_read':[k for k in m['files']if not k.endswith(('.jpg','.png'))],'upstream_tests':34,'dependency_files_pinned':len(deps),'dependency_scope':'Original test source/images/expected hashes unchanged; original wildcard dependency ranges and jpeg-js0.0.4/png-js0.1.1 resolved in separate test-dependency prefix with own pinned lock. Full dev install failed browserify-shim registry404; browser build/lint not qualified. No lifecycle installation scripts executed.','findings':[{'source':'index.js:hammingDistance','finding':'Invalid hex zz versus00 returns distance0 because parseInt NaN enters JavaScript bitwise coercion. Typed validated hashes required; do not adopt this public string validation.'},{'source':'index.js:bmvbhash_even','finding':'Method1 floors block dimensions, so on5x5/bits4 a changed fifth row is omitted; method2 detects it. Method1 documented for divisible sizes only, requiring explicit policy.'},{'source':'index.js:bmvbhash','finding':'Fractional method2 block weighting agrees with an independent exact-Fraction pixel/block intersection oracle on the two5x5 edge fixtures.'},{'source':'index.js:alpha branches','finding':'Fully transparent RGB ignored via white fill, while partial alpha1 and opaque255 share identical hashes for same RGB. This fingerprint is not full visible-pixel equality.'}],'license':'MIT code license read; fixture provenance separately lists public sources; no production code or image fixtures copied into rrrah.','adoptable':['Explicit divisible-size policy for fast paths','Independent weighted-block area oracle as a requirement','Never use perceptual hash distance as final identity'],'not_adopted':['Unchecked hex parse/coercion','Partial-alpha-insensitive duplicate acceptance','Unbounded decoding/blocks without cancellation/source guards'],'pins':{str(p):sha(p)for p in paths},'scope':'Deepens registry362: complete pinned text source/config/test/readme/license and reference-vector review, all blobs verified,34unchanged original tests on macOSNode plus edge controls. Not browser/backend/platform/resource/precision or all500 completion.'};out.write_text(json.dumps(report,indent=2)+'\n');print(report['status'],report['files'],report['upstream_tests'])
if __name__=='__main__':main()
