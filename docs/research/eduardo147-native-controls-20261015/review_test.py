"""Original unmodified Python module, real owned files and SQLite; no web listener."""
from pathlib import Path
import hashlib,importlib.util,os,sqlite3,tempfile,unittest
from unittest.mock import patch
ROOT=Path(__file__).resolve().parents[3]
SRC=ROOT/'docs/research/dedup-500/evidence/eduardoklosowski--deduplicated/full-source'
bootstrap=tempfile.TemporaryDirectory(prefix='rrrah-eduardo147-import-')
original_expanduser=os.path.expanduser
spec=importlib.util.spec_from_file_location('review147_original',SRC/'deduplicated/__init__.py')
up=importlib.util.module_from_spec(spec)
with patch('os.path.expanduser',side_effect=lambda p:bootstrap.name if p=='~' else original_expanduser(p)):
 spec.loader.exec_module(up)
class Review(unittest.TestCase):
 def setUp(self):
  self.temp=tempfile.TemporaryDirectory(prefix='rrrah-eduardo147-files-');self.root=Path(self.temp.name);self.cache=self.root/'cache';self.cache.mkdir();self.files=self.root/'files';self.files.mkdir();up.CACHE_DIR=str(self.cache);self.directory=up.Directory(str(self.files))
 def tearDown(self): self.directory._conn.close();self.temp.cleanup()
 def write(self,name,data): p=self.files/name;p.write_bytes(data);return p
 def update(self):
  self.directory.update_tree()
  for name in self.directory.hash_for_update(): self.directory.update_hash(name)
 def test_full_sha1_stream_boundaries(self):
  for size in (0,1,1023,1024,1025,4097,65537):
   data=bytes((i*17+3)%256 for i in range(size));p=self.write('input',data)
   self.assertEqual(up.sha1_file(str(p)),hashlib.sha1(data).hexdigest())
 def test_same_size_mtime_mutation_keeps_stale_digest(self):
  a=self.write('a',b'aaaa');self.write('b',b'aaaa');self.update();old=a.stat();a.write_bytes(b'bbbb');os.utime(a,ns=(old.st_atime_ns,old.st_mtime_ns))
  self.assertEqual(self.directory.update_tree(),(0,0,0));self.assertEqual(len(list(self.directory.get_duplicated())),1)
  self.assertNotEqual(up.sha1_file(str(a)),up.sha1_file(str(self.files/'b')))
 def test_query_trusts_stale_catalog(self):
  a=self.write('a',b'aaaa');self.update();a.write_bytes(b'bbbb');query=self.root/'query';query.write_bytes(b'aaaa')
  self.assertEqual(self.directory.is_file_in(str(query)),[str(a)])
 def test_web_eager_generator_order_leaves_stats_stale(self):
  self.write('a',b'aaaa');self.write('b',b'aaaa');self.directory.update_tree()
  names=list(self.directory.hash_for_update())
  for name in names: self.directory.update_hash(name)
  self.assertTrue(self.directory.is_completed());self.assertEqual(len(list(self.directory.get_duplicated())),1)
  self.assertEqual(self.directory.get_duplicated_hash(),0)
 def test_lazy_cli_generator_order_updates_stats(self):
  self.write('a',b'aaaa');self.write('b',b'aaaa');self.update()
  self.assertEqual(self.directory.get_duplicated_hash(),1);self.assertEqual(self.directory.get_duplicated_files(),2);self.assertEqual(self.directory.get_duplicated_size(),4)
 def test_injected_digest_collision_is_not_byte_confirmed(self):
  self.write('a',b'aaaa');self.write('b',b'bbbb');self.directory.update_tree()
  with patch.object(up,'sha1_file',return_value='controlled-not-a-real-sha1-collision'):
   for name in self.directory.hash_for_update(): self.directory.update_hash(name)
  rows=list(self.directory.get_duplicated());self.assertEqual(rows[0][2],['a','b'])
 def test_failed_owned_removal_deletes_row_in_open_transaction(self):
  p=self.write('a',b'aaaa');self.update()
  with patch.object(up.os,'remove',side_effect=PermissionError('owned removal injection')):
   with self.assertRaises(PermissionError): self.directory.delete_file('a')
  self.assertTrue(p.exists());self.directory._db.execute('SELECT count(*) FROM files');self.assertEqual(self.directory._db.fetchone()[0],0)
  self.directory._conn.rollback();self.directory._db.execute('SELECT count(*) FROM files');self.assertEqual(self.directory._db.fetchone()[0],1)
 def test_pre_hash_stat_can_describe_different_bytes(self):
  p=self.write('a',b'aaaa');self.directory.update_tree();old=p.stat();real=up.sha1_file
  def replace_then_hash(name): p.write_bytes(b'longer-new-content');return real(name)
  with patch.object(up,'sha1_file',side_effect=replace_then_hash): self.directory.update_hash('a')
  self.directory._db.execute('SELECT size,hash FROM files WHERE filename=?',('a',));size,digest=self.directory._db.fetchone()
  self.assertEqual(size,old.st_size);self.assertNotEqual(size,p.stat().st_size);self.assertEqual(digest,real(str(p)))
if __name__=='__main__':
 try: unittest.main(verbosity=2)
 finally: bootstrap.cleanup()
