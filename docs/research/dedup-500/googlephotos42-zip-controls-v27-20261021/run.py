from pathlib import Path
import hashlib, subprocess, zipfile, warnings, json
root=Path.cwd()
out=root/'.dedup-work/googlephotos42-20261021/zip-controls'
out.mkdir(parents=True,exist_ok=True)
script=root/'.dedup-work/googlephotos42-20261021/scripts/zip-content-digest.sh'
data=script.read_bytes()
tree=json.loads((root/'docs/research/dedup-500/evidence/SylphxAI--Google-Photos-Delete-Tool/tree.json').read_text())['tree']
expected=next(t['sha'] for t in tree if t['path']=='scripts/zip-content-digest.sh')
assert hashlib.sha1(b'blob '+str(len(data)).encode()+b'\0'+data).hexdigest()==expected
def archive(name, entries):
 p=out/name
 with warnings.catch_warnings():
  warnings.simplefilter('ignore',UserWarning)
  with zipfile.ZipFile(p,'w') as z:
   for key,payload in entries:
    z.writestr(zipfile.ZipInfo(key,(2024,1,1,0,0,0)),payload)
 return p
def digest(p):
 r=subprocess.run(['bash',str(script),str(p)],capture_output=True,text=True,check=True)
 return r.stdout.strip()
a=archive('first.zip',[('same.txt',b'A'),('same.txt',b'BC')])
b=archive('second.zip',[('same.txt',b'AB'),('same.txt',b'C')])
with zipfile.ZipFile(a) as z: first=[z.read(i) for i in z.infolist()]
with zipfile.ZipFile(b) as z: second=[z.read(i) for i in z.infolist()]
assert first==[b'A',b'BC'] and second==[b'AB',b'C']
assert a.read_bytes()!=b.read_bytes()
assert digest(a)==digest(b)
print('PASS 1: different duplicate-name entry payloads produce identical original content digest')
c=archive('unique-a.zip',[('a',b'A'),('b',b'BC')])
d=archive('unique-b.zip',[('a',b'AB'),('b',b'C')])
assert digest(c)!=digest(d)
print('PASS 2: unique-name content change is distinguished')
print('duplicate digest:',digest(a))
