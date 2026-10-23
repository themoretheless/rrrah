import ast,contextlib,csv,io,itertools,json
from pathlib import Path
source=Path('.dedup-work/examples088-20261024/pgsql_big_dedupe_example/pgsql_big_dedupe_example.py')
tree=ast.parse(source.read_text());nodes=[n for n in tree.body if isinstance(n,(ast.ClassDef,ast.FunctionDef))];ns={'csv':csv,'io':io,'itertools':itertools};exec(compile(ast.Module(body=nodes,type_ignores=[]),str(source),'exec'),ns)
rows=[('simple',1),('comma,quote"',2),('line\nfeed',3),('кириллица',4),('z'*100000,5),('',6)]
checks=[]
for chunk_rows in [1,2,3,10000]:
 reader=ns['Readable'](iter(rows));parts=[]
 while True:
  part=reader.read(chunk_rows)
  if not part:break
  parts.append(part)
 parsed=list(csv.reader(io.StringIO(''.join(parts))));assert parsed==[[str(x) for x in row] for row in rows]
 checks.append({'requested_size':chunk_rows,'chunks':len(parts),'max_returned_characters':max(map(len,parts)),'roundtrip_rows':len(parsed)})
with contextlib.redirect_stdout(io.StringIO()):
 pairs=list(ns['record_pairs']([(7,{'name':'a'},9,{'name':'b'})]))
assert pairs==[((7,{'name':'a'}),(9,{'name':'b'}))]
clusters=list(ns['cluster_ids']([((7,9),(.9,.8)),((12,),(.7,))]));assert clusters==[(7,7,.9),(9,7,.8),(12,12,.7)]
print(json.dumps({'scope':'Original pure class/functions parsed unchanged from pinned source; no database imports or client/server COPY protocol exercised.','readable_roundtrip':checks,'record_pair_adapter':pairs,'cluster_id_adapter':clusters,'conclusion':'read(size) limits rows, not characters or bytes; a single admitted row exceeds the nominal size by over 100000 characters. CSV escaping and row roundtrip passed four chunk sizes.'},ensure_ascii=False,indent=2))
