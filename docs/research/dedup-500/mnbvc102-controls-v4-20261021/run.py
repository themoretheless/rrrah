from pathlib import Path
import ast,importlib.util,tempfile,hashlib,os
R=Path(__file__).resolve().parents[4];C=R/'.dedup-work/mnbvc102-20261021';p=R/'docs/research/dedup-500/evidence/aplmikex--deduplication_mnbvc/04-customSimhash.py'
spec=importlib.util.spec_from_file_location('original_textsimhash',p);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
t=ast.parse((C/'convert/convert.py').read_text());t.body=[n for n in t.body if isinstance(n,ast.FunctionDef) and n.name=='from_txt_to_json'];ns={'os':os,'hashlib':hashlib,'customSimhash':m};exec(compile(t,'original_txt_converter','exec'),ns)
with tempfile.TemporaryDirectory(prefix='mnbvc-text-') as d:
 a=Path(d)/'a.txt';b=Path(d)/'b.txt';a.write_text(' alpha \n\nalpha\nbeta\n');b.write_text('alpha\nbeta\n')
 x=ns['from_txt_to_json'](str(a),0.8);y=ns['from_txt_to_json'](str(b),0.8)
 assert x['simhash']==y['simhash'] and a.read_bytes()!=b.read_bytes()
 assert [p['内容'] for p in x['段落']]==['alpha','alpha','beta']
 assert [p['是否重复'] for p in x['段落']]==[False,True,False]
 assert x['段落数']==3 and x['去重段落数']==2 and x['是否待查文件']
 print('PASS original normalization strips whitespace, marks repeated paragraph, hashes unique paragraphs equally despite different file bytes')
 a.write_text(' \n\t\n');assert ns['from_txt_to_json'](str(a),0.8) is None
 print('PASS blank-only input yields explicit None')
 a.write_bytes(bytes([255]))
 try:ns['from_txt_to_json'](str(a),0.8)
 except UnicodeDecodeError:print('PASS invalid UTF8 explicitly raises')
 else:raise AssertionError('Expected UTF8 decode error')
print('3 original converter controls passed; no process pool, corpus or external dependencies beyond available NumPy')
