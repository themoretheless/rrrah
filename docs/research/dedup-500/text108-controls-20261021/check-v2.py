import ast,pathlib,json,hashlib,re,subprocess,sys,os,itertools
import numpy as np
source=pathlib.Path('.dedup-work/text108-20261021/dedup_toolkit.py');b=source.read_bytes();names={'normalize_text','_get_simhash_features','get_document_simhash','_construct_suffix_array_doubling','_construct_lcp_array_py','clean_text'};tree=ast.parse(b);ns={'np':np,'re':re};exec(compile(ast.Module(body=[n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name in names],type_ignores=[]),str(source),'exec'),ns)
if '--child' in sys.argv:
 print(json.dumps({text:ns['get_document_simhash'](text,128).tolist() for text in ['alpha','beta','alpha beta gamma','hello world','one two three four']}));sys.exit(0)
seed_results={}
for seed in ['1','2']:
 env=dict(os.environ,PYTHONHASHSEED=seed);p=subprocess.run([sys.executable,__file__,'--child'],capture_output=True,text=True,env=env,check=True);seed_results[seed]=json.loads(p.stdout)
assert seed_results['1']!=seed_results['2']
assert all(sig[1] in [0,(1<<64)-1] for result in seed_results.values() for sig in result.values())
checked=0
for n in range(10):
 for seq in itertools.product(range(2),repeat=n):
  data=bytes(seq);sa=ns['_construct_suffix_array_doubling'](data);oracle=sorted(range(n),key=lambda i:data[i:]);assert sa==oracle
  lcp=ns['_construct_lcp_array_py'](data,sa);expected=[0]*n
  for k in range(1,n):
   a,c=sa[k-1],sa[k];h=0
   while a+h<n and c+h<n and data[a+h]==data[c+h]:h+=1
   expected[k]=h
  assert lcp==expected,(data,lcp,expected);checked+=1
try:ns['clean_text'](None)
except UnboundLocalError: none_error='UnboundLocalError'
else:raise AssertionError()
r={'clean_text_none':none_error,'python_source_sha256':hashlib.sha256(b).hexdigest(),'method':'Six original Python function AST nodes compiled unchanged with real NumPy/re; no heavy module imports, native core, fake Faiss or dataset adapters. Child processes use fixedPYTHONHASHSEED1/2.','simhash_seed_results':seed_results,'different_across_hash_seeds':True,'upper64_bits_always_uniform_in10_cases':True,'suffix_array_and_lcp':{'cases':checked,'alphabet':[0,1],'lengths':[0,9],'oracle':'Independent suffix-slice sort and direct character LCP scan','mismatches':0},'limits':'No AVX2 execution or timing; native vector sign discrepancy only source/header inspection. Python SimHash output not claimed native recipe parity.'};pathlib.Path('docs/research/dedup-500/text108-controls-20261021/result-v2.json').write_text(json.dumps(r,indent=2)+'\n');print(json.dumps(r,indent=2))
