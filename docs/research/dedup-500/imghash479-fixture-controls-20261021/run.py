import hashlib,json,subprocess
from pathlib import Path
root=Path('/Users/themoretheless/.codex/tmp/rrrah-kevingliewe-imghash-source-20261013')
binary=Path('/Users/themoretheless/.codex/tmp/rrrah-kevingliewe-imghash-qualified-20261013/imghash')
assert hashlib.sha256(binary.read_bytes()).hexdigest()=='ebb30e9c637e0fc7e563f86f47e4e1b647e4539539aea0022446682ccf5aab64'
rows=[]
for alg in ['Mean','Gradient','VertGradient','DoubleGradient','Blockhash']:
 for name in ['Lenna.png','Lenna_blocked.png','Lenna_noisy.png','Peppers.png']:
  args=['-a',alg,str(root/'testdata/Lenna.png'),str(root/'testdata'/name)]
  p=subprocess.run([str(binary),*args],capture_output=True,text=True,timeout=30)
  assert p.returncode==0,(args,p.stdout,p.stderr)
  d=int(p.stdout.strip()); assert 0<=d<=64
  if name=='Lenna.png': assert d==0
  rows.append(dict(algorithm=alg,comparison=name,distance=d,returncode=p.returncode,stderr=p.stderr))
result=dict(executable_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),controls=rows,scope='20 real original CLI comparisons; 2 overlap historical Gradient self/different controls,18 additional distinct controls. No precision calibration or equality admission.')
Path(__file__).with_name('result.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2))
