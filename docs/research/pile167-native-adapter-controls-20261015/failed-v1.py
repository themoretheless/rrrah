"""Original selected functions/classes; owned files/pickle/mmap and explicit unavailable-backend adapters."""
from pathlib import Path
import ast,glob,io,json,logging,math,mmap,os,pickle,re,signal,sys,tempfile,types,unittest
from unittest.mock import patch
ROOT=Path(__file__).resolve().parents[3]
SRC=ROOT/'docs/research/dedup-500/evidence/EleutherAI--pile_dedupe/full-source'
def load(filename,names,ns):
 parsed=ast.parse((SRC/filename).read_text());nodes=[n for n in parsed.body if isinstance(n,(ast.FunctionDef,ast.ClassDef)) and n.name in names];assert len(nodes)==len(names)
 exec(compile(ast.Module(body=nodes,type_ignores=[]),filename,'exec'),ns);return ns
logger=logging.getLogger('review167')
class Progress:
 def __init__(self,**kwargs):pass
 def __enter__(self):return self
 def __exit__(self,*args):pass
 def update(self,*args):pass
 def close(self):pass
progress=types.SimpleNamespace(tqdm=Progress)
text=load('archiver.py',{'TextReader'},dict(mmap=mmap,os=os,tqdm=progress))
dups=load('working_with_duplicates.py',{'yield_duplicates','get_duplicate_stats'},dict(os=os,glob=glob,pickle=pickle,json=json,logger=logger,duplicates_statistics_file='duplicate_statistics.json'))
dedup=load('dedupe.py',{'load_or_create_lsh','save_duplicate_batch','ensure_disk_space'},dict(os=os,pickle=pickle,logger=logger))
mini=load('working_with_minhashes.py',{'yield_minhashes'},dict(os=os,glob=glob,pickle=pickle,re=re))
yielding=load('yield_deduped_pile.py',{'yield_deduped_pile'},dict(get_duplicate_stats=dups['get_duplicate_stats'],logger=logger))
stats=load('working_with_pile.py',{'get_file_stats'},dict(os=os))
generation=load('generate_minhashes.py',{'process_batch'},dict(os=os,pickle=pickle,signal=signal,SIGINT=signal.SIGINT,SIG_IGN=signal.SIG_IGN,Path=Path,sys=sys,logger=logger,generate_minhash=lambda *_:None))
class Review(unittest.TestCase):
 def setUp(self):self.tmp=tempfile.TemporaryDirectory(prefix='rrrah-pile167-owned-');self.root=Path(self.tmp.name)
 def tearDown(self):self.tmp.cleanup()
 def write_pickle(self,name,value):p=self.root/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(pickle.dumps(value));return p
 def test_unterminated_utf8_line_loses_last_character_in_all_readers(self):
  p=self.root/'lines.txt';p.write_text('alpha\nβeta',encoding='utf8');reader=text['TextReader'](str(p))
  for method in ['read','read_slow','read_tqdm']:self.assertEqual(list(getattr(reader,method)()),['alpha','βet'])
  rows=list(reader.read_and_tell());self.assertEqual([r[0] for r in rows],['alpha','βet']);self.assertEqual(sum(r[1] for r in rows),len(p.read_bytes()))
 def test_empty_mmap_reader_refuses_but_slow_reader_is_empty(self):
  p=self.root/'empty.txt';p.write_bytes(b'');reader=text['TextReader'](str(p));self.assertEqual(list(reader.read_slow()),[])
  with self.assertRaises(ValueError):list(reader.read())
 def test_crlf_mmap_and_text_mode_readers_disagree(self):
  p=self.root/'crlf.txt';p.write_bytes(b'alpha\r\n');reader=text['TextReader'](str(p));self.assertEqual(list(reader.read()),['alpha\r']);self.assertEqual(list(reader.read_slow()),['alpha'])
 def test_advertised_generator_fails_on_missing_stats_argument(self):
  with self.assertRaisesRegex(TypeError,'duplicates_directory'):next(yielding['yield_deduped_pile'](str(self.root),str(self.root)))
 def test_smol_parent_directory_filters_full_duplicate_files(self):
  p=self.write_pickle('smol-parent/duplicates_0000.pkl',[(3,[3,5])]);self.assertEqual(list(dups['yield_duplicates'](str(p.parent))),[])
  p=self.write_pickle('normal/duplicates_0000.pkl',[(3,[3,5])]);self.assertEqual(list(dups['yield_duplicates'](str(p.parent))),[(3,[3,5])])
 def test_minhash_files_numeric_order_and_bad_name(self):
  self.write_pickle('minhashes_20.pkl',[(20,'twenty')]);self.write_pickle('minhashes_3.pkl',[(3,'three')]);self.assertEqual(list(mini['yield_minhashes'](str(self.root))),[(3,'three'),(20,'twenty')])
  self.write_pickle('minhashes_bad.pkl',[])
  with self.assertRaises(AttributeError):list(mini['yield_minhashes'](str(self.root)))
 def test_old_lsh_pickle_ignores_requested_threshold_and_source(self):
  p=self.write_pickle('lsh.pkl',dict(old_threshold=.2,old_source='owned-old'));self.assertEqual(dedup['load_or_create_lsh'](str(p),.9,999,'owned-new'),dict(old_threshold=.2,old_source='owned-old'))
 def test_pile_character_stats_count_meta_tuple_length(self):
  p=self.root/'source';p.write_bytes(b'owned')
  class Reader:
   def __init__(self):self.fh=io.BytesIO()
   def read(self,*args,**kwargs):yield ('abcdefgh',{});yield ('x'*100,{})
  stats['Reader']=Reader
  self.assertEqual(stats['get_file_stats'](str(p),Progress,Progress()),(2,4))
 def test_second_duplicate_write_failure_leaves_inconsistent_pair(self):
  real=pickle.dump;calls=[]
  def dump(value,stream):
   calls.append(value)
   if len(calls)==2:raise OSError('owned simulated ENOSPC')
   return real(value,stream)
  with patch.object(pickle,'dump',side_effect=dump):
   with self.assertRaises(OSError):dedup['save_duplicate_batch'](str(self.root),[(3,[3,5])],0)
  self.assertEqual(pickle.loads((self.root/'duplicates_0000.pkl').read_bytes()),[(3,[3,5])]);self.assertEqual((self.root/'duplicates_smol_0000.pkl').read_bytes(),b'')
 def test_transaction_failure_leaves_sigint_ignored_and_lock(self):
  class Pool:
   def map(self,*args):return ['owned-minhash']
  old=signal.getsignal(signal.SIGINT)
  try:
   with patch.object(pickle,'dump',side_effect=OSError('owned write failure')):
    with self.assertRaises(OSError):generation['process_batch'](Pool(),None,[(0,'owned')],str(self.root),'')
   self.assertEqual(signal.getsignal(signal.SIGINT),signal.SIG_IGN);self.assertTrue((self.root/'.transaction_lock').exists())
  finally:signal.signal(signal.SIGINT,old)
if __name__=='__main__':unittest.main(verbosity=2)
