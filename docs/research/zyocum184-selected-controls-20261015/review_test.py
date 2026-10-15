"""Original selected functions; synthetic precomputed fingerprints, no CityHash backend."""
from pathlib import Path
import ast,csv,difflib,io,itertools,math,operator,sys,tempfile,types,unittest
from contextlib import redirect_stdout
ROOT=Path(__file__).resolve().parents[3]
SRC=ROOT/'docs/research/dedup-500/evidence/zyocum--dedup/full-source'
def load(filename,names,ns):
 parsed=ast.parse((SRC/filename).read_text());functions=[n for n in parsed.body if isinstance(n,ast.FunctionDef) and n.name in names];assert len(functions)==len(names)
 exec(compile(ast.Module(body=functions,type_ignores=[]),filename,'exec'),ns);return ns
def placeholder_hash(text): raise AssertionError('CityHash was not qualified or executed')
dedup=load('dedup.py',{'ngrams','rotate','document_sorter','simdiff','pairs'},dict(CityHash128=placeholder_hash,HASHSIZE={placeholder_hash:32},combinations=itertools.combinations,itemgetter=operator.itemgetter))
equiv=load('list_equivalence_classes.py',{'main','filter_rows'},dict(csv=csv,sys=sys))
matches=load('matching_substrings.py',{'matching_substrings'},dict(SequenceMatcher=difflib.SequenceMatcher,attrgetter=operator.attrgetter))
naive=load('dedup_pairwise.py',{'diff','combos'},dict(difflib=difflib,factorial=math.factorial))
class Document:
 def __init__(self,n): self.lsh=n;self.filename=str(n);self.size=1
class Review(unittest.TestCase):
 def test_rotate_exhaustive_small_oracle(self):
  for value in range(256):
   bits=f'{value:08b}'
   for rotation in range(-8,17):
    shift=rotation%8;expected=int(bits[-shift:]+bits[:-shift],2) if shift else value
    self.assertEqual(dedup['rotate'](value,rotation,width=8),expected)
 def test_simdiff_all128_positions(self):
  for bit in range(128): self.assertEqual(dedup['simdiff'](0,1<<bit,bits=128),1)
 def test_iterator_consumed_after_first_rotation(self):
  docs=[Document(n) for n in range(4)]
  as_list=list(dedup['pairs'](docs));as_iterator=list(dedup['pairs'](iter(docs)))
  list_pairs={(a.filename,b.filename) for (a,b),score in as_list};iterator_pairs={(a.filename,b.filename) for (a,b),score in as_iterator}
  self.assertEqual(len(iterator_pairs),3);self.assertGreater(len(list_pairs),3);self.assertLess(iterator_pairs,list_pairs)
 def test_window_larger_than_input_drops_every_pair(self):
  self.assertEqual(list(dedup['pairs']([Document(0),Document(0)],window=3)),[])
 def test_equivalence_bridge_leaves_overlapping_classes(self):
  with tempfile.TemporaryDirectory() as d:
   p=Path(d)/'pairs.tsv';p.write_text('a\tb\t0\nc\td\t0\nb\tc\t0\n');sink=io.StringIO()
   with redirect_stdout(sink): equiv['main'](str(p),0,False)
   groups={}
   for group,name in csv.reader(io.StringIO(sink.getvalue()),delimiter='\t'):groups.setdefault(group,set()).add(name)
   self.assertEqual(len(groups),2);g=list(groups.values());self.assertTrue(g[0]&g[1]);self.assertEqual(set.union(*g),{'a','b','c','d'})
 def test_repetitive_common_substring_hidden_by_autojunk(self):
  self.assertEqual(list(matches['matching_substrings']('b'+'a'*200,'c'+'a'*200,threshold=5)),[])
  oracle=difflib.SequenceMatcher(None,'b'+'a'*200,'c'+'a'*200,autojunk=False).find_longest_match()
  self.assertEqual(oracle.size,200)
 def test_zero_threshold_includes_empty_sentinel(self):
  result=list(matches['matching_substrings']('a','b',threshold=0));self.assertEqual(len(result),1);self.assertEqual(result[0].size,0)
 def test_empty_pair_diff_raises(self):
  with self.assertRaises(ZeroDivisionError): naive['diff']('','')
if __name__=='__main__':unittest.main(verbosity=2)
