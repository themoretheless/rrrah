"""Selected original methods after Python2 syntax conversion; explicit backend adapter.
Not native Python2, BlitzDB, multiprocessing or CLI qualification.
"""
from pathlib import Path
import ast, functools, io, json, os, sys, tempfile, threading, types, unittest
from lib2to3.refactor import RefactoringTool, get_fixers_from_package
from PIL import Image as PILImage
ROOT=Path(__file__).resolve().parents[3]
SRC=ROOT/'docs/research/dedup-500/evidence/beeftornado--duplicate-image-finder/full-source'
tool=RefactoringTool(get_fixers_from_package('lib2to3.fixes'))
converted=str(tool.refactor_string((SRC/'duplicateimagefinder/app.py').read_text(),'app.py'))
parsed=ast.parse(converted)
original=next(n for n in parsed.body if isinstance(n,ast.ClassDef) and n.name=='ImageUtils')
methods=[n for n in original.body if isinstance(n,ast.FunctionDef)]
selected=ast.Module(body=[ast.ClassDef(name='ImageUtils',bases=[],keywords=[],body=methods,decorator_list=[])],type_ignores=[])
class Document:
 class DoesNotExist(Exception): pass
 class MultipleDocumentsReturned(Exception): pass
 def __init__(self, data): self.__dict__.update(data)
class Backend:
 def __init__(self): self.record=None
 def get(self,*args):
  if self.record is None: raise Document.DoesNotExist()
  return self.record
 def save(self,record): self.record=record
 def commit(self): pass
 def delete(self,record): self.record=None
image_api=types.SimpleNamespace(Image=PILImage.Image,open=PILImage.open,ANTIALIAS=PILImage.Resampling.LANCZOS)
ns=dict(Image=image_api,ImageHash=Document,os=os,reduce=functools.reduce)
exec(compile(ast.fix_missing_locations(selected),'selected-converted-upstream-app.py','exec'),ns)
U=ns['ImageUtils'];U.backend_lock=threading.Lock()
base=types.SimpleNamespace();exec(compile((SRC/'duplicateimagefinder/output_formats/base.py').read_text(),'upstream-base.py','exec'),base.__dict__)
formatter_source=(SRC/'duplicateimagefinder/output_formats/jsono.py').read_text()
formatter_tree=ast.parse(formatter_source)
formatter_ns=dict(base=base,json=json,sys=sys,ListType=list)
exec(compile(ast.Module(body=[n for n in formatter_tree.body if isinstance(n,ast.ClassDef)],type_ignores=[]),'upstream-jsono.py','exec'),formatter_ns)
class Review(unittest.TestCase):
 def setUp(self): U.saved_hashes={};U.persistent_store=Backend();U.new_hash_count=0
 def test_black_and_white_both_zero(self):
  self.assertEqual(U.hash(PILImage.new('L',(8,9),0)),0)
  self.assertEqual(U.hash(PILImage.new('L',(8,9),255)),0)
 def test_actual_hash_uses_bits_above_64(self):
  im=PILImage.new('L',(8,9),0)
  for x in range(8): im.putpixel((x,8),255)
  value=U.hash(im);self.assertEqual(value,255<<64);self.assertEqual(value.bit_length(),72)
 def test_hamming_all72bits_and_python2_score_formula(self):
  distance=U.hamming_score(0,(1<<72)-1);self.assertEqual(distance,72)
  self.assertEqual((64-distance)*100//64,-13)
 def test_72pixel_sum_divided_by64_discards_low_gray(self):
  im=PILImage.new('L',(8,9),100);im.putpixel((0,0),101)
  self.assertEqual(U.hash(im),0)
 def test_memory_hit_ignores_file_mutation_and_existence(self):
  U.saved_hashes['owned-missing-path']=123
  self.assertEqual(U.hash(None,'owned-missing-path'),123)
 def test_future_timestamp_accepts_stale_persistent_hash(self):
  with tempfile.TemporaryDirectory() as d:
   p=Path(d)/'owned.txt';p.write_text('changed non-image');mtime=p.stat().st_mtime
   U.persistent_store.record=Document(dict(name=str(p),hash=321,created=mtime+100))
   self.assertEqual(U.hash(str(p),str(p)),321)
 def test_nonempty_json_output_is_invalid(self):
  class Sink(io.StringIO):
   def old_write(self,value): return self.write(value)
  for count in (1,2):
   sink=Sink();previous=sys.stdout
   try:
    sys.stdout=sink;formatter_ns['JsonFormat']().output([base.OutputRecord('a','b',0,100)]*count)
   finally: sys.stdout=previous
   with self.assertRaises(json.JSONDecodeError): json.loads(sink.getvalue())
 def test_empty_json_is_valid(self):
  class Sink(io.StringIO):
   def old_write(self,value): return self.write(value)
  sink=Sink();previous=sys.stdout
  try: sys.stdout=sink;formatter_ns['JsonFormat']().output([])
  finally: sys.stdout=previous
  self.assertEqual(json.loads(sink.getvalue()),[])
if __name__=='__main__': unittest.main(verbosity=2)
