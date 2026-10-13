"""Real pinned unchanged imghash CLI controls with actual image-rs decoding."""
from pathlib import Path
import subprocess,json,hashlib,re
T=Path('/Users/themoretheless/.codex/tmp');S=T/'rrrah-kevingliewe-imghash-source-20261013';E=T/'rrrah-kevingliewe-imghash-target-20261013/debug/imghash'
Q=T/'rrrah-kevingliewe-imghash-qualified-20261013';Q.mkdir();subprocess.run(['cp','-cp',str(E),str(Q/'imghash')],check=True);E=Q/'imghash'
from PIL import Image
for name,color in [('black',0),('white',255)]:Image.new('RGB',(8,8),(color,)*3).save(Q/(name+'.png'))
a=str(S/'testdata/Lenna.png');b=str(S/'testdata/Peppers.png');missing=str(Q/'absent.png')
cases=[('hash',[a]),('self',[a,a]),('different',[a,b]),('flat_collision',[str(Q/'black.png'),str(Q/'white.png')]),('lowercase_alg',['-a','gradient',a]),('invalid_width',['-w','abc',a]),('invalid_height',['-h','abc',a]),('missing_first',[missing]),('missing_second',[a,missing]),('zero_width',['-w','0',a])]
rows=[]
for name,args in cases:
 p=subprocess.run([str(E),*args],capture_output=True,text=True,timeout=15);rows.append(dict(control=name,args=args,returncode=p.returncode,stdout=p.stdout,stderr=p.stderr))
assert rows[0]['returncode']==0 and re.fullmatch('[0-9a-f]{16}\n',rows[0]['stdout'])
assert rows[1]['stdout']=='0\n' and rows[1]['returncode']==0
assert rows[2]['returncode']==0 and int(rows[2]['stdout'])>0
assert rows[3]['stdout']=='0\n' and rows[3]['returncode']==0
assert [r['returncode'] for r in rows[4:9]]==[4,1,2,6,7]
P=Path('docs/research/dedup-500/kevingliewe-imghash-controls-20261013.json');assert not P.exists();P.write_text(json.dumps(dict(status='executed_ten_real_cli_controls',source=str(S),executable=str(E),executable_sha256=hashlib.sha256(E.read_bytes()).hexdigest(),script_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),controls=rows,scope='Unchanged pinned source built with original Cargo.lock; actual image-rs/img_hash backends; zero-width outcome recorded, not presumed'),indent=2)+'\n');print([(r['control'],r['returncode'],r['stdout'].strip()) for r in rows])
