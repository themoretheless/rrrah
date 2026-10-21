from pathlib import Path
import ast,numpy as np,itertools,json,tempfile,os
source=Path('docs/research/dedup-500/evidence/deveyNull--phistOfFury/02-phist.py').read_text();tree=ast.parse(source)
names={'_binary_array_to_hex','hamming1','directoryEater'};defs=[n for n in tree.body if isinstance(n,ast.FunctionDef)and n.name in names]
ns={'np':np,'numpy':np,'itertools':itertools,'getcwd':os.getcwd,'listdir':os.listdir,'rename':os.rename,'join':os.path.join};exec(compile(ast.Module(body=defs,type_ignores=[]),'original phist functions AST','exec'),ns)
rows=[]
for length,tail in [(100,4),(25,1)]:
 a=np.zeros(length,dtype=bool);b=a.copy();b[-tail:]=True
 left=ns['_binary_array_to_hex'](a);right=ns['_binary_array_to_hex'](b);assert left==right
 rows.append({'name':'truncated_tail','input_bits':length,'different_tail_bits':tail,'serialized_hex_digits':len(left),'same_serialization':True})
try:ns['hamming1']('00','ff');raise AssertionError('expected Python3 failure')
except AttributeError as e:rows.append({'name':'python3_hamming','observed':str(e)})
original=Path.cwd()
with tempfile.TemporaryDirectory(prefix='phist374-')as tmp:
 p=Path(tmp);target=p/'query';target.mkdir();(target/'fixture.txt').write_text('controlled');(p/'unrelated file.txt').write_text('controlled')
 try:
  os.chdir(p);got=ns['directoryEater'](str(target));assert (p/'unrelated-file.txt').exists() and not (p/'unrelated file.txt').exists();assert len(got)==1
  rows.append({'name':'directory_scan_renames_cwd','observed':'Scanning query subdirectory renamed controlled unrelated file in parent current directory'})
 finally:os.chdir(original)
Path(__file__).with_name('result.json').write_text(json.dumps({'repository':'deveyNull/phistOfFury','commit':'96598bf78c4f33bfca68f4b3fc9c4b334f175644','controls':rows,'scope':'Unchanged original functions extracted through AST, actual NumPy/Python3 and temporary filesystem; no whole module import or image decoder','limits':['No original tests exist in tracked tree inventory.','Full source/assets review still incomplete; no claim about historical Python/Pillow runtime.']},indent=2)+'\n');print(json.dumps(rows))
