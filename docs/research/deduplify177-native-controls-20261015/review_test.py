"""Unmodified package with real pinned TinyDB/JMESPath dependencies and owned files."""
from pathlib import Path
import hashlib,io,os,sys,tempfile,unittest
from contextlib import redirect_stdout,redirect_stderr
from unittest.mock import patch
from tinydb import TinyDB
import deduplify.hash_files as hashes
import deduplify.compare_files as compare
import deduplify.del_empty_dirs as clean
import deduplify.cli as cli
class Review(unittest.TestCase):
 def setUp(self):self.temp=tempfile.TemporaryDirectory(prefix='rrrah-deduplify177-owned-');self.root=Path(self.temp.name)
 def tearDown(self):self.temp.cleanup()
 def write(self,path,data=b'owned'):p=self.root/path;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(data);return p
 def test_full_md5_stream_boundaries(self):
  for size in (0,1,65535,65536,65537,131072,131073):
   data=bytes((i*13+5)%256 for i in range(size));p=self.write('input',data);digest,out=hashes.hashfile(str(p));self.assertEqual(digest,hashlib.md5(data).hexdigest());self.assertEqual(out,str(p))
 def test_zero_block_size_hashes_nonempty_file_as_empty(self):
  p=self.write('input',b'owned nonempty');digest,_=hashes.hashfile(str(p),0);self.assertEqual(digest,hashlib.md5(b'').hexdigest());self.assertNotEqual(digest,hashlib.md5(p.read_bytes()).hexdigest())
 def test_restart_skips_different_file_with_same_basename(self):
  a=self.write('a/same.txt',b'first');b=self.write('b/same.txt',b'different');dbpath=self.root/'catalog.json'
  with TinyDB(str(dbpath)) as db:db.insert({'hash':hashes.hashfile(str(a))[0],'filepath':str(a),'duplicate':False})
  with redirect_stdout(io.StringIO()):hashes.run_hash(str(b.parent),1,str(dbpath),restart=True)
  with TinyDB(str(dbpath)) as db:self.assertEqual(len(db),1);self.assertEqual(db.all()[0]['filepath'],str(a))
  self.assertNotEqual(a.read_bytes(),b.read_bytes())
 def test_stale_catalog_returns_changed_file_as_delete_candidate(self):
  a=self.write('same.txt',b'initial equal');b=self.write('deeper/same.txt',b'initial equal');digest=hashes.hashfile(str(a))[0]
  with TinyDB(str(self.root/'catalog.json')) as db:
   db.insert_multiple([dict(hash=digest,filepath=str(p),duplicate=True) for p in (a,b)]);b.write_bytes(b'changed unique owned bytes')
   self.assertEqual(compare.compare_filenames(digest,db),[str(b)])
  self.assertNotEqual(a.read_bytes(),b.read_bytes());self.assertTrue(b.exists())
 def test_worker_deletion_failure_is_not_reported(self):
  p=self.write('owned.txt')
  with patch.object(compare.os,'remove',side_effect=PermissionError('owned failure injection')):
   with self.assertLogs(compare.logger,level='INFO') as logs:
    result=compare.delete_files([str(p)],1)
  self.assertIsNone(result);self.assertTrue(p.exists());self.assertTrue(any('Deletion complete!' in message for message in logs.output))
 def test_parse_args_ignores_supplied_argument_list(self):
  with patch.object(sys,'argv',['review','compare','--infile',str(self.root/'catalog.json')]):args=cli.parse_args(['hash',str(self.root)])
  self.assertEqual(args.command,'compare')
  with patch.object(sys,'argv',['review','hash',str(self.root),'--exts','xml']),redirect_stderr(io.StringIO()):
   with self.assertRaises(SystemExit):cli.parse_args([])
 def test_accepted_global_extension_option_does_not_filter_hashing(self):
  a=self.write('input/a.xml');b=self.write('input/b.txt');dbpath=self.root/'catalog.json'
  with patch.object(sys,'argv',['review','--exts','xml','hash',str(a.parent),'--dbfile',str(dbpath)]):args=cli.parse_args([])
  self.assertEqual(args.exts,'xml')
  with redirect_stdout(io.StringIO()):args.func(**vars(args))
  with TinyDB(str(dbpath)) as db:self.assertEqual({row['filepath'] for row in db.all()},{str(a.resolve()),str(b.resolve())})
 def test_invalid_worker_count_deletes_existing_owned_catalog_first(self):
  p=self.write('owned-catalog.json',b'{"owned_previous_catalog":true}');directory=self.root/'input';directory.mkdir()
  with patch.object(sys,'argv',['review','hash',str(directory),'--dbfile',str(p),'--count',str(cli.CPUS+1)]):
   with self.assertRaises(ValueError):cli.main()
  self.assertFalse(p.exists())
 def test_empty_directory_cleanup_leaves_newly_emptied_parent(self):
  tree=self.root/'tree';leaf=tree/'a'/'b';leaf.mkdir(parents=True)
  with redirect_stdout(io.StringIO()):clean.empty_dir_search(str(tree))
  self.assertFalse(leaf.exists());self.assertTrue((tree/'a').exists());self.assertEqual(list((tree/'a').iterdir()),[])
if __name__=='__main__':unittest.main(verbosity=2)
