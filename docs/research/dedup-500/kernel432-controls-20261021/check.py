import ast, hashlib, json, math, pathlib
source=pathlib.Path('.dedup-work/kernel432-20261021/KernelPHash.py')
b=source.read_bytes();tree=ast.parse(b)
names={'TextHash64_real','HammingDist64_real','HammingDist64_oneside','HammingDist64'}
ns={'math':math}
exec(compile(ast.Module(body=[n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name in names],type_ignores=[]),str(source),'exec'),ns)
a='abcdefgh'+'ijklmnop';c='ijklmnop'+'abcdefgh'
ha=ns['TextHash64_real'](a);hc=ns['TextHash64_real'](c)
assert a!=c and ha==hc
width=ns['HammingDist64_real']('10000000000000000','10000000000000001')
assert width==0
try:ns['HammingDist64']([],[])
except ZeroDivisionError: empty='ZeroDivisionError'
else:raise AssertionError()
r={'source_sha256':hashlib.sha256(b).hexdigest(),'method':'Original four function AST nodes compiled unchanged; module imports and top-level shell commands excluded. No native pHash execution.','window_permutation_collision':{'inputs':[a,c],'hashes':[ha,hc],'distance':ns['HammingDist64'](ha,hc)},'overwidth_65_bit_input':{'observed':width,'integer_xor_popcount':1},'empty_lists':empty,'limits':'Synthetic controls establish counterexamples only, not end-to-end parser or retrieval quality.'}
p=pathlib.Path('docs/research/dedup-500/kernel432-controls-20261021/result.json');p.write_text(json.dumps(r,indent=2)+'\n');print(p.read_text())
