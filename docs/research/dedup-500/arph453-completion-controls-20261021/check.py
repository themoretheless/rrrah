from pathlib import Path
import sys,ast,types,json,hashlib,math
sys.path.insert(0,str(Path('.dedup-work/arph453-review-deps-20261021').resolve()))
import msgpack,numpy as np
root=Path('.dedup-work/arph453-20261021');p=root/'arph/data/log_threshold.msgpack';data=msgpack.unpackb(p.read_bytes(),raw=False)
assert set(data)=={'input_length','threshold','pedersen','hash_list'}
prime=2**256-2**32-977
points=[]
for group in ['pedersen','hash_list']:
 for h in data[group]:
  assert isinstance(h,str) and len(h)==128
  x,y=int(h[:64],16),int(h[64:],16)
  assert 0<=x<prime and 0<=y<prime and (y*y-x*x*x-7)%prime==0
  points.append(h)
n,t=data['input_length'],data['threshold'];expected=sum(math.comb(n,w)*2**w for w in range(1,t+1));assert len(data['pedersen'])==n and len(data['hash_list'])==expected
source=(root/'experiments/uniformity.py').read_text();tree=ast.parse(source);defs=[x for x in tree.body if isinstance(x,ast.FunctionDef)]
ns={'np':np,'category_ids':['synthetic'],'images':[['zero','full','half']],'arph':types.SimpleNamespace(underlying=types.SimpleNamespace(Image=lambda x:x))}
exec(compile(ast.Module(body=defs,type_ignores=[]),'original uniformity functions AST','exec'),ns)
hasher=types.SimpleNamespace(hash=lambda name:{'zero':'00','full':'ff','half':'0f'}[name]);actual=ns['test_uniformity'](hasher)['synthetic'];assert actual[2]==4 and actual[1]==8
r={'repository':'sambux1/arph','commit':'32ebc32c3572252573071b158f0bd81255b03d41','msgpack_version':msgpack.__version__,'description':{'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'input_length':n,'threshold':t,'pedersen_points':len(data['pedersen']),'error_points':len(data['hash_list']),'expected_nonzero_signed_error_vectors':expected,'all_points_on_secp256k1':True,'unique_encoded_points':len(set(points))},'uniformity_control':{'original_functions_via_AST':True,'controlled_hash_weights':[0,8,4],'reported_mean_max_min':[float(x) for x in actual],'true_min':0},'limits':['Curve equation checks establish valid encoded points, not origin, independence or cryptographic strength.','No ecpy/full package/original tests run; imagehash,cv2,pdqhash absent.','Uniformity uses controlled image/hash adapters; no TinyImageNet dataset evaluation.']}
Path(__file__).with_name('result.json').write_text(json.dumps(r,indent=2)+'\n');print(json.dumps(r))
