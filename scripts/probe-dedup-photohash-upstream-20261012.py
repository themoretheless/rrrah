#!/usr/bin/env python3
"""Original pinned tests, explicit Pillow compatibility and counterexamples."""
import hashlib,importlib,io,json,subprocess,sys,unittest
from pathlib import Path
from PIL import Image
import PIL
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';T=Path('/Users/themoretheless/.codex/tmp');S=T/'rrrah-photohash-full-source-20261012';COMMIT='2a43d23a879f1484cd59775f940853eb38168b17'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'photohash-upstream-controls-20261012.json';mp=D/'photohash-full-source-20261012.json';assert not out.exists()and not mp.exists();assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=S,text=True).strip()==COMMIT
 rows=[]
 for line in subprocess.check_output(['git','ls-tree','-r','HEAD'],cwd=S,text=True).splitlines():
  meta,name=line.split('\t');mode,kind,blob=meta.split();assert kind=='blob';p=S/name;data=p.read_bytes();assert hashlib.sha1(f'blob {len(data)}\0'.encode()+data).hexdigest()==blob;rows.append({'path':name,'mode':mode,'git_blob':blob,'bytes':len(data),'sha256':sha(p)})
 manifest={'repository':'bunchesofdonald/photohash','commit':COMMIT,'source':str(S),'files':rows};mp.write_text(json.dumps(manifest,indent=2)+'\n');sys.path.insert(0,str(S));module=importlib.import_module('photohash.tests.test_photohash');ph=importlib.import_module('photohash.photohash')
 def suite():
  stream=io.StringIO();r=unittest.TextTestRunner(stream=stream,verbosity=2).run(unittest.defaultTestLoader.loadTestsFromModule(module));return {'tests':r.testsRun,'failures':len(r.failures),'errors':len(r.errors),'skipped':len(r.skipped),'log':stream.getvalue()}
 original=suite();adapter='none'
 if not hasattr(Image,'ANTIALIAS'):Image.ANTIALIAS=Image.Resampling.LANCZOS;adapter='Pillow Image.ANTIALIAS alias only; algorithm and original tests unchanged'
 compatible=suite();scratch=T/'rrrah-photohash-controls-20261012';assert not scratch.exists();scratch.mkdir();paths={}
 for name,image in [('black',Image.new('RGB',(8,8),'black')),('white',Image.new('RGB',(8,8),'white')),('invisible_black',Image.new('RGBA',(8,8),(0,0,0,0)))]:p=scratch/(name+'.png');image.save(p);paths[name]=str(p)
 image=Image.new('RGBA',(8,8));image.putdata([(i*4,i*4,i*4,0)for i in range(64)]);p=scratch/'invisible_gradient.png';image.save(p);paths['invisible_gradient']=str(p)
 hashes={k:ph.average_hash(v)for k,v in paths.items()};decoded=[]
 for p in (S/'photohash/tests/assets').glob('*.jpg'):
  with Image.open(p)as image:image.load();decoded.append({'path':str(p),'mode':image.mode,'size':image.size,'sha256':sha(p),'hash':ph.average_hash(p)})
 controls={'hex_0_f_reported_distance':ph.hash_distance('0','f'),'hex_0_f_bit_distance':4,'six_hex_character_mismatches_are_similar':ph.hashes_are_similar('0'*16,'f'*6+'0'*10),'six_hex_character_mismatches_bit_distance':24,'hashes':hashes,'opaque_black_white_similar':ph.is_look_alike(paths['black'],paths['white']),'fully_invisible_pair_distance':ph.distance(paths['invisible_black'],paths['invisible_gradient'])}
 assert all(sha(S/x['path'])==x['sha256']for x in rows)
 r={'repository':manifest['repository'],'commit':COMMIT,'python':sys.version,'pillow':PIL.__version__,'original_suite':original,'compatibility':adapter,'compatible_suite':compatible,'decoded_original_fixtures':decoded,'controls':controls,'pins':{str(p):sha(p)for p in [mp,Path(__file__),Path(Image.__file__)]},'scope':'Original source/tests unchanged; missing legacy Pillow constant adapted only in second run. Development hash/alpha counterexamples, no backend/platform/resource/precision qualification or production copying.'};out.write_text(json.dumps(r,indent=2)+'\n');print({'original':(original['tests'],original['failures'],original['errors']),'compatible':(compatible['tests'],compatible['failures'],compatible['errors']),'controls':controls})
if __name__=='__main__':main()
