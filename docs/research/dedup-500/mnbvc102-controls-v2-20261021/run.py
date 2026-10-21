"""Original producer/validator and utilities; synthetic CSVs in owned temp dir."""
from pathlib import Path
from types import SimpleNamespace as N
import ast,csv,queue,tempfile,importlib.util,hashlib
R=Path(__file__).resolve().parents[4];C=R/'.dedup-work/mnbvc102-20261021';E=R/'docs/research/dedup-500/evidence/aplmikex--deduplication_mnbvc'
def module(p,name):
 spec=importlib.util.spec_from_file_location(name,p);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
m=module(E/'04-customSimhash.py','original_simhash');u=module(C/'utils/utils.py','original_utilities')
t=ast.parse((E/'03-multiprocess_deduplication.py').read_text());t.body=[n for n in t.body if isinstance(n,ast.FunctionDef)]
ns={'customSimhash':m,'tqdm':N(tqdm=lambda x:x),'csv':csv,'jaccard_distance':u.jaccard_distance}
exec(compile(t,'original_producer_validator','exec'),ns)
with tempfile.TemporaryDirectory(prefix='mnbvc-candidate-') as d:
 paths=[str(Path(d)/f'{i}.csv') for i in range(3)]
 def write(p,tokens):
  with open(p,'w',newline='') as f:
   w=csv.writer(f);w.writerow([0,'synthetic.jsonl',Path(p).name,0]);w.writerow(tokens)
 for p in paths:write(p,['initial'])
 q=queue.Queue();flag=N(value=False)
 ns['deduplication'](paths,None,0,q,flag)
 pairs=list(q.queue);assert len(pairs)==2
 selected=next(pair[1] for pair in pairs if pair[0]==paths[2])
 alternate=next(p for p in paths[:2] if p!=selected)
 write(selected,['A']);write(alternate,['B']);write(paths[2],['B'])
 assert u.jaccard_distance({'B'},{'B'})==1
 ns['check_similar_file'](q,0.8,flag)
 assert Path(paths[2]).read_text().startswith('0,')
 print('PASS original one-candidate flow misses authored alternate Jaccard1match after selected Jaccard0rejection')
 q.put([paths[2],alternate]);ns['check_similar_file'](q,0.8,flag)
 assert Path(paths[2]).read_text().startswith('1,')
 print('PASS original validator marks same query when alternate candidate explicitly supplied')
 try:u.jaccard_distance(set(),set())
 except ZeroDivisionError:print('PASS original empty-set Jaccard raises ZeroDivisionError')
 else:raise AssertionError('Expected empty union exception')
print('3 controls passed; exact synthetic intermediate fingerprints, no claimed natural text Simhash collision or full multiprocess execution')
