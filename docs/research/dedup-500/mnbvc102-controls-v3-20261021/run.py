"""Original output-writer AST with JSON-lines reader adapter and owned temp files."""
from pathlib import Path
from types import SimpleNamespace as N
import ast,json,csv,tempfile,hashlib,os,contextlib,errno
R=Path(__file__).resolve().parents[4];C=R/'.dedup-work/mnbvc102-20261021'
t=ast.parse((C/'parallel_dedup/write_output_to_jsonl.py').read_text());t.body=[n for n in t.body if isinstance(n,ast.FunctionDef)]
@contextlib.contextmanager
def reader(p):
 with open(p,encoding='utf-8') as f:yield (json.loads(line) for line in f)
with tempfile.TemporaryDirectory(prefix='mnbvc-output-') as d:
 root=Path(d);records=root/'json';records.mkdir();csvs=root/'csv';csvs.mkdir()
 p=records/'records.jsonl';row={'文件名':'synthetic.txt','payload':'authored 中文','是否重复文件':False}
 initial=json.dumps(row).encode()+b'\n';p.write_bytes(initial)
 ns={'os':os,'hashlib':hashlib,'json':json,'jsonlines':N(open=reader),'tqdm':N(tqdm=lambda x:x),
 'tempfile':N(NamedTemporaryFile=lambda **kw:tempfile.NamedTemporaryFile(dir=root,**kw)),
 'get_all_files':lambda *args:([str(p)],1)}
 exec(compile(t,'original-output-writer','exec'),ns)
 name=hashlib.md5(str(p).encode()).hexdigest()+hashlib.md5(row['文件名'].encode()).hexdigest()+'.csv'
 (csvs/name).write_text('1,synthetic\n')
 ns['write_output_to_jsonl'](str(csvs),str(records));out=json.loads(p.read_text())
 assert out==dict(row,是否重复文件=True)
 print('PASS successful writer marks duplicate and preserves authored fields')
 p.write_bytes(initial);(csvs/name).unlink()
 try:ns['write_output_to_jsonl'](str(csvs),str(records))
 except FileNotFoundError:assert p.read_bytes()==initial;print('PASS missing CSV propagates while original JSONL remains intact')
 else:raise AssertionError('Expected missing CSV failure')
 (csvs/name).write_text('1,synthetic\n')
 def refused(*a):raise OSError(errno.EXDEV,'authored cross-device refusal')
 ns['os']=N(path=os.path,replace=refused)
 try:ns['write_output_to_jsonl'](str(csvs),str(records))
 except OSError as e:
  assert e.errno==errno.EXDEV and p.read_bytes()==initial
  print('PASS injected cross-device publication failure propagates and preserves original JSONL')
 else:raise AssertionError('Expected publication failure')
print('3 controls passed; JSON-lines dependency adapted, no real corpus/deletion or multiprocessing')
