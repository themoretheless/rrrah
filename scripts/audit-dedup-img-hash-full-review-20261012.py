#!/usr/bin/env python3
import hashlib,json,re
from fractions import Fraction as F
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'img-hash-full-review-audit-20261012.json';assert not out.exists();rp=D/'img-hash-full-review-20261012.json';r=json.loads(rp.read_text());assert all(sha(p)==v for p,v in r['pins'].items());mp=D/r['source_manifest'];m=json.loads(mp.read_text());assert len(m['files'])==13;s=Path(m['source'])
 for row in m['files']:
  p=s/row['path'];b=p.read_bytes();assert len(b)==row['bytes']and sha(p)==row['sha256'];assert hashlib.sha1(f'blob {len(b)}\0'.encode()+b).hexdigest()==row['git_blob']
 assert 'MIT OR Apache-2.0'in(s/'Cargo.toml').read_text();assert r['unit_tests_passed']==r['doc_tests_passed']==3 and r['doc_compile_only']==1;assert re.search(r'fn test_transpose\(\)\s*\{\s*\}',(s/'src/dct.rs').read_text())
 # Independent exact overlap integration of one white pixel occupying[1,2]^2.
 side=F(9,8);blocks=[]
 for y in range(8):
  for x in range(8):
   dx=max(F(0),min(F(2),(x+1)*side)-max(F(1),x*side));dy=max(F(0),min(F(2),(y+1)*side)-max(F(1),y*side));blocks.append(255*dx*dy)
 bits=[]
 for start in [0,32]:
  group=blocks[start:start+32];median=sorted(group)[16];bits.extend(v>median or(v==median and median>255*side*side/2)for v in group)
 expected=[sum(int(bits[i+j])<<j for j in range(8))for i in range(0,64,8)];assert expected==[3,3,0,0,0,0,0,0];c=r['controls'];assert c['fractional_hash']==[1,0,0,0,0,0,0,0]and c['fractional_hash']!=expected;assert c['unequal_length_distance']==0 and c['transparent_mean_distance']==32 and c['transparent_block_distance']==c['black_white_mean_distance']==0
 out.write_text(json.dumps({'status':'verified_full_img_hash_source_original_default_tests_and_fractional_counterexample','registry_index':358,'git_blobs':13,'text_files_read':13,'unit_tests':3,'vacuous_unit_tests':1,'doc_tests':3,'compile_only_doc_tests':1,'fractional_exact_oracle':expected,'fractional_upstream':c['fractional_hash'],'pins':{str(p):sha(p)for p in [rp,mp,Path(__file__)]},'scope':'Complete pinned13file source/licenses/config/benchmark review; original default3units(oneempty)+3docs(onecompile-only), pinned native counterexamples and independent Fraction area integration. No source copying, nightly/bench/oldplatform/resource/precision/full500 completion.'},indent=2)+'\n');print('Verified full13file img_hash review and exact fractional-block counterexample')
if __name__=='__main__':main()
