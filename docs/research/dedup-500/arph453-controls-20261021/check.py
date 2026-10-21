from pathlib import Path
import importlib.util,ast,types,json,hashlib
import numpy as np
root=Path('.dedup-work/arph453-20261021')
spec=importlib.util.spec_from_file_location('review_common',root/'arph/common.py');common=importlib.util.module_from_spec(spec);spec.loader.exec_module(common)
x=np.array([1,0,1,0,1,0,1,0,1],dtype=np.uint8);h=common.np_binary_to_hex(x);y=common.hex_to_np_binary(h);assert len(x)==9 and len(y)==16
source=(root/'arph/pph/nonrobust.py').read_text();tree=ast.parse(source);cls=next(n for n in tree.body if isinstance(n,ast.ClassDef) and n.name=='Nonrobust');method=next(n for n in cls.body if isinstance(n,ast.FunctionDef)and n.name=='evaluate');module=ast.Module(body=[method],type_ignores=[]);ns={'np':np};exec(compile(module,'original Nonrobust.evaluate AST','exec'),ns)
accepted=bool(ns['evaluate'](types.SimpleNamespace(tau=1),'0000000000000000','ffffffffffffffff'));assert accepted
result={'repository':'sambux1/arph','commit':'32ebc32c3572252573071b158f0bd81255b03d41','numpy':np.__version__,'controls':[{'name':'nonbyte_roundtrip','input_bits':len(x),'hex':h,'output_bits':len(y),'original_common_module':True},{'name':'hex_string_evaluation','actual_accepts':accepted,'true_bit_distance':64,'tau':1,'scope':'Unchanged original evaluate method extracted via Python AST; no full package import or cryptographic backend'}],'source_sha256':{str(p):hashlib.sha256(p.read_bytes()).hexdigest()for p in [root/'arph/common.py',root/'arph/pph/nonrobust.py']},'limits':['No cryptographic strength or original full-suite claim.','Nine-bit roundtrip padding is normal NumPy packing; API fails to preserve logical bit length.','Method extraction isolates existing input-type behavior, not original random projection generation.']}
Path(__file__).with_name('result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result['controls']))
