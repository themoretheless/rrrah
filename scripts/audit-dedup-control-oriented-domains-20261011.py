#!/usr/bin/env python3
"""JPEG SOF plus primary EXIF orientation for control-domain audit."""
import hashlib,json,struct
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def orientation(path):
 data=Path(path).read_bytes();assert data[:2]==b'\xff\xd8';i=2;found=[]
 while i<len(data):
  assert data[i]==255;i+=1
  while data[i]==255:i+=1
  marker=data[i];i+=1
  if marker==218:break
  if marker in [1,*range(208,218)]:continue
  n=int.from_bytes(data[i:i+2],'big');assert n>=2
  segment=data[i+2:i+n]
  if marker==225 and segment.startswith(b'Exif\0\0'):
   t=segment[6:];assert t[:2] in [b'II',b'MM'];end='<' if t[:2]==b'II' else '>';assert struct.unpack_from(end+'H',t,2)[0]==42;off=struct.unpack_from(end+'I',t,4)[0];count=struct.unpack_from(end+'H',t,off)[0]
   for j in range(count):
    at=off+2+12*j;tag,kind,length=struct.unpack_from(end+'HHI',t,at)
    if tag==274:
     assert kind==3 and length==1;v=struct.unpack_from(end+'H',t,at+8)[0];assert 1<=v<=8;found.append(v)
  i+=n
 assert len(set(found))<=1;return found[0] if found else 1

def main():
 out=D/'dedup-control-oriented-domains-audit-20261011.json';assert not out.exists();rp=D/'dedup-anchor-scale-corpus-controls-20261011.json';r=json.loads(rp.read_text());pins={str(rp):sha(rp),str(Path(__file__)):sha(__file__)};inventory=[]
 for item in r['inventory']:
  assert sha(item['target'])==item['sha256'];o=orientation(item['target']);raw=item['dimensions'];actual=raw[::-1] if o>=5 else raw;inventory.append({'target':item['target'],'raw_dimensions':raw,'orientation':o,'oriented_dimensions':actual,'changed_domain':raw!=actual});pins[item['target']]=item['sha256']
 out.write_text(json.dumps({'status':'verified_primary_exif_control_domains','pins':pins,'inventory':inventory,'scope':'Raw JPEG dimensions corrected for primary EXIF orientation. Validated against real bounded decode for201100; other files require decoder-domain parity before correction completion claim. Prior experiment captures retained, not rewritten.'},indent=2)+'\n');print([(Path(x['target']).name,x['orientation'],x['oriented_dimensions']) for x in inventory if x['changed_domain']])
if __name__=='__main__':main()
