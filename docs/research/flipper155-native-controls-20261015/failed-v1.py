"""Unmodified stdlib-only file deduplicator; only public fixtures/owned synthetic IO."""
from pathlib import Path
import hashlib,importlib.util,io,json,os,shutil,subprocess,sys,tempfile,unittest
from contextlib import redirect_stdout
ROOT=Path(__file__).resolve().parents[3]
SRC=ROOT/'docs/research/dedup-500/evidence/ratticon--flipper-signal-deduplicator/full-source'
spec=importlib.util.spec_from_file_location('original_flipper155',SRC/'signal_dedup.py');up=importlib.util.module_from_spec(spec);spec.loader.exec_module(up)
class Review(unittest.TestCase):
 def setUp(self):self.temp=tempfile.TemporaryDirectory(prefix='rrrah-flipper155-owned-');self.root=Path(self.temp.name)
 def tearDown(self):self.temp.cleanup()
 def write(self,path,data=b'owned'):p=self.root/path;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(data);return p
 def copy(self,groups,path):
  with redirect_stdout(io.StringIO()):return up.copyUnique(groups,str(path),overwrite=True)
 def test_full_hash_4096_boundaries(self):
  for size in (0,1,4095,4096,4097,8192,8193):
   data=bytes((i*13+7)%256 for i in range(size));p=self.write('input',data);self.assertEqual(up.getHash(str(p)),hashlib.md5(data).hexdigest())
 def test_all_original_sample_groups_preserve_membership(self):
  files=up.getHashes(str(SRC/'sample-data'));groups=up.groupDuplicateHashes(files);self.assertEqual(len(files),7);self.assertEqual(sorted(map(len,groups.values())),[3,4]);self.assertEqual({p for paths in groups.values() for p in paths},set(files))
  for digest,paths in groups.items():
   for path in paths:self.assertEqual(hashlib.md5(Path(path).read_bytes()).hexdigest(),digest)
 def test_full_cli_on_owned_copy_of_public_samples(self):
  sample=self.root/'input';shutil.copytree(SRC/'sample-data',sample);output=self.root/'output';p=subprocess.run([sys.executable,str(SRC/'signal_dedup.py'),'-i',str(sample),'-o',str(output),'-y'],capture_output=True,text=True,cwd=self.root,env=dict(os.environ,PYTHONDONTWRITEBYTECODE='1'))
  self.assertEqual(p.returncode,0,p.stdout+p.stderr);self.assertIn('Reduced 7 files to 2',p.stdout);paths=list(output.iterdir());self.assertEqual(len(paths),2);self.assertEqual({up.getHash(str(p)) for p in paths},set(up.getHashes(str(sample)).values()));self.assertEqual(len(list(sample.rglob('*.sub'))),7)
 def test_two_unique_same_basenames_overwrite_one_destination(self):
  a=self.write('a/same.sub',b'first');b=self.write('b/same.sub',b'second');groups=up.groupDuplicateHashes({str(p):up.getHash(str(p)) for p in (a,b)});self.assertEqual(len(groups),2);output=self.root/'output';self.assertTrue(self.copy(groups,output));self.assertEqual([p.name for p in output.iterdir()],['same.sub']);self.assertEqual((output/'same.sub').read_bytes(),b'second')
 def test_changed_source_copied_under_old_hash(self):
  a=self.write('a.sub',b'initial');old=up.getHash(str(a));groups={old:[str(a)]};a.write_bytes(b'changed owned bytes');output=self.root/'output';self.assertTrue(self.copy(groups,output));self.assertNotEqual(up.getHash(str(output/'a.sub')),old)
 def test_hidden_existing_output_survives_clear(self):
  a=self.write('input/a.sub');output=self.root/'output';self.write('output/.owned-sentinel',b'old');self.assertTrue(self.copy({up.getHash(str(a)):[str(a)]},output));self.assertTrue((output/'.owned-sentinel').exists());self.assertEqual(len(list(output.iterdir())),2)
 def test_output_subdirectory_causes_uncaught_removal_error(self):
  a=self.write('input/a.sub');output=self.root/'output';(output/'owned-child').mkdir(parents=True)
  with self.assertRaises(IsADirectoryError):self.copy({up.getHash(str(a)):[str(a)]},output)
  self.assertTrue((output/'owned-child').exists())
 def test_previous_output_deleted_before_missing_source_error(self):
  old=self.write('output/old.sub',b'owned previous publication');output=old.parent
  with self.assertRaises(FileNotFoundError):self.copy({'owned-stale-hash':[str(self.root/'missing.sub')]},output)
  self.assertFalse(old.exists());self.assertEqual(list(output.iterdir()),[])
 def test_absolute_custom_output_path_is_not_excluded_from_discovery(self):
  a=self.write('input/a.sub');old=self.write('input/custom/old.sub',b'owned old output');files=up.getHashes(str(a.parent),exclude=['output',str(old.parent)]);self.assertIn(str(old),files)
 def test_overlapping_source_output_destroys_only_owned_source_before_error(self):
  a=self.write('overlap/a.sub',b'owned synthetic source');groups={up.getHash(str(a)):[str(a)]}
  with self.assertRaises(FileNotFoundError):self.copy(groups,a.parent)
  self.assertFalse(a.exists())
if __name__=='__main__':unittest.main(verbosity=2)
