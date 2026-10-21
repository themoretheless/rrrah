"""Execute extracted original SQL in SQLite; not a SQLiteCpp/MVP runtime test."""
from pathlib import Path
import re,json,sqlite3
root=Path(__file__).resolve().parents[4]
s=(root/'.dedup-work/imagehash472-20261021/mvptable.cpp').read_text()
section=s[s.index('MVPTable::blob_type MVPTable::find_vantage_point'):s.index('std::vector<int64_t> MVPTable::check_balance')]
strings=re.findall(r'"(?:[^"\\]|\\.)*"',section)
joined=''.join(json.loads(x) for x in strings)
# Select only the two contiguous literal SQL blocks, preserving C++ concatenation.
blocks=re.findall(r'((?:"(?:[^"\\]|\\.)*"\s*)+)',section)
queries=[''.join(json.loads(x) for x in re.findall(r'"(?:[^"\\]|\\.)*"',b)) for b in blocks]
queries=[q for q in queries if q.startswith(('SELECT value FROM (','WITH sampled_points'))]
assert len(queries)==2
c=sqlite3.connect(':memory:')
c.create_function('mvp_distance',2,lambda a,b:abs(a-b))
c.execute('CREATE TABLE mvp_points(id INTEGER PRIMARY KEY, partition INTEGER,value INTEGER)')
c.executemany('INSERT INTO mvp_points VALUES(?,?,?)',[(i,0,i) for i in range(1,27)])
for i,q in enumerate(queries):
 try:c.execute(q,{'sample_size':25}).fetchall()
 except sqlite3.OperationalError as e:
  assert 'syntax error' in str(e),str(e)
  print('PASS original SQL branch',i,'fails:',e)
 else:raise AssertionError('Original malformed SQL unexpectedly accepted')
# Explicit diagnostic adaptation: insert just the missing separator.
fixed=[q.replace('sum_distFROM','sum_dist FROM') for q in queries]
assert c.execute(fixed[0]).fetchone()[0] in (1,26)
print('PASS separator-only adaptation makes full-pair branch execute')
try:c.execute(fixed[1],{'sample_size':25}).fetchall()
except sqlite3.OperationalError as e:
 assert 'no such column: p.id' in str(e),str(e)
 print('PASS separator-only sampled adaptation exposes missing p.id:',e)
else:raise AssertionError('Missing sampled id unexpectedly accepted')
print('4 controls passed; no original C++ MVP/SQLiteCpp or decoder runtime qualification')
