#!/usr/bin/env python3
import hashlib,json
from pathlib import Path
from PIL import Image
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'photohash-full-review-audit-20261012.json';assert not out.exists();rp=D/'photohash-full-review-20261012.json';r=json.loads(rp.read_text());assert all(sha(p)==v for p,v in r['pins'].items());mp=D/r['source_manifest'];m=json.loads(mp.read_text());s=Path(m['source']);assert len(m['files'])==18
 for row in m['files']:
  p=s/row['path'];b=p.read_bytes();assert len(b)==row['bytes']and sha(p)==row['sha256'];assert hashlib.sha1(f'blob {len(b)}\0'.encode()+b).hexdigest()==row['git_blob']
 assert r['text_files_read']==[x['path']for x in m['files']if not x['path'].endswith('.jpg')]and len(r['text_files_read'])==14
 cp=D/r['controls'];c=json.loads(cp.read_text());assert all(sha(p)==v for p,v in c['pins'].items());assert all(sha(p)==v for p,v in c['fixture_pins'].items());assert c['original_suite']['tests']==5 and c['original_suite']['errors']==3 and c['original_suite']['failures']==0 and 'ANTIALIAS'in c['original_suite']['log'];assert c['compatible_suite']['tests']==5 and c['compatible_suite']['errors']==c['compatible_suite']['failures']==0;assert len(c['decoded_original_fixtures'])==4
 for row in c['decoded_original_fixtures']:assert sha(row['path'])==row['sha256']
 v=c['controls'];assert v['hex_0_f_reported_distance']==1 and (int('0',16)^int('f',16)).bit_count()==4;assert v['six_hex_character_mismatches_are_similar']and (int('0'*16,16)^int('f'*6+'0'*10,16)).bit_count()==24;assert v['opaque_black_white_similar']and v['hashes']['black']==v['hashes']['white'];assert v['fully_invisible_pair_distance']==8
 invisible=[Path(p)for p in c['fixture_pins']if Path(p).name.startswith('invisible_')];assert len(invisible)==2
 for p in invisible:
  with Image.open(p)as image:assert image.convert('RGBA').getchannel('A').getextrema()==(0,0)
 assert 'MIT License'in (s/'LICENSE.txt').read_text()
 out.write_text(json.dumps({'status':'verified_full_photohash_source_and_bounded_upstream_runtime_review','registry_index':360,'git_blobs':18,'text_files_read':14,'original_jpeg_fixtures':4,'original_tests':5,'compatible_passed':5,'unadapted_errors':3,'pins':{str(p):sha(p)for p in [rp,mp,cp,Path(__file__)]},'scope':'Complete pinned repository tree, unchanged original tests with explicit legacy-Pillow constant adapter and independent bit/alpha controls. No production copying/fullplatform/oldbackend/precision/RSS/full500 completion.'},indent=2)+'\n');print('Verified18blob full photohash review and5compatible original tests')
if __name__=='__main__':main()
