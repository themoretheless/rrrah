import sys,ast,types,importlib.util,os,tempfile,sqlite3,json,hashlib,unittest
from pathlib import Path
ROOT=Path(os.environ.get('RRRAH_REPO',Path(__file__).resolve().parents[2]))
C=ROOT/'.dedup-work/immich15-20261021'
sys.path.insert(0,str(ROOT/'.dedup-work/immich15-deps'))
import numpy as np,faiss
from PIL import Image
import PIL
spec=importlib.util.spec_from_file_location('original_db',C/'db.py');db=importlib.util.module_from_spec(spec);spec.loader.exec_module(db)
def original_function(file,name,namespace):
 tree=ast.parse((C/file).read_text());node=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name==name);exec(compile(ast.Module(body=[node],type_ignores=[]),str(C/file),'exec'),namespace);return namespace[name]
class Stub:
 def __init__(self):self.session_state={};self.errors=[]
 def write(self,*a,**k):pass
 def button(self,*a,**k):return False
 def empty(self,*a,**k):return self
 def progress(self,*a,**k):return self
 def text(self,*a,**k):pass
 def error(self,*a,**k):self.errors.append(a)
class Controls(unittest.TestCase):
 def setUp(self):self.temp=tempfile.TemporaryDirectory();self.previous=os.getcwd();os.chdir(self.temp.name);db.startup_processed_duplicate_faiss_db()
 def tearDown(self):os.chdir(self.previous);self.temp.cleanup()
 def pairs(self):
  with sqlite3.connect('duplicates.db') as c:return c.execute('SELECT vector_id1,vector_id2,similarity FROM duplicates ORDER BY id').fetchall()
 def generate(self,points,ids):
  index=faiss.IndexFlatL2(2);index.add(np.asarray(points,dtype='float32'));st=Stub();fn=original_function('imageDuplicate.py','generate_db_duplicate',{'np':np,'st':st,'init_or_load_faiss_index':lambda:(index,ids),'save_duplicate_pair':db.save_duplicate_pair});fn();return st
 def test_singleton_creates_self_pair_from_missing_neighbor(self):
  self.generate([[0,0]],['only']);self.assertEqual(self.pairs()[0][:2],('only','only')); self.assertGreater(self.pairs()[0][2],10)
 def test_three_equal_vectors_do_not_enumerate_all_three_pairs(self):
  self.generate([[0,0],[0,0],[0,0]],['a','b','c']);pairs={frozenset(r[:2]) for r in self.pairs()};self.assertLess(len(pairs),3);self.assertEqual(len(pairs),2)
 def test_saved_distance_is_not_updated(self):
  db.save_duplicate_pair('a','b',.1);db.save_duplicate_pair('a','b',.9);self.assertEqual(self.pairs(),[('a','b',.1)])
 def test_deleting_one_pair_leaves_other_asset_references(self):
  db.save_duplicate_pair('a','b',.1);db.save_duplicate_pair('a','c',.2);db.delete_duplicate_pair('a','b');self.assertEqual(self.pairs(),[('a','c',.2)])
 def test_grayscale_is_not_normalized_to_rgb(self):
  fn=original_function('imageDuplicate.py','convert_image_to_rgb',{});image=Image.new('L',(2,2));self.assertIs(fn(image),image);self.assertEqual(fn(Image.new('RGBA',(2,2))).mode,'RGB')
 def test_formatted_file_sizes_compare_lexically(self):
  fn=original_function('utility.py','compare_and_color',{});a=db.bytes_to_megabytes(10*1024*1024);b=db.bytes_to_megabytes(2*1024*1024);self.assertIn('red',fn(a,b));self.assertEqual(a,'10.000 MB')
 def test_null_exif_is_not_handled_as_missing_metadata(self):
  fn=original_function('api.py','getAssetInfo',{'bytes_to_megabytes':db.bytes_to_megabytes})
  with self.assertRaises(TypeError):fn('a',[{'id':'a','exifInfo':None}])
if __name__=='__main__':
 print('Actual dependency versions: numpy='+np.__version__+' faiss='+faiss.__version__+' pillow='+PIL.__version__,flush=True)
 unittest.main(verbosity=2)
