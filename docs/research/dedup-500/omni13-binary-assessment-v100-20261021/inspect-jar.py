from pathlib import Path
import zipfile,struct,hashlib,json,io
C=Path('.dedup-work/omni13-20261021');b=(C/'android--gradle--wrapper--gradle-wrapper.jar').read_bytes();rows=[]
class Reader:
 def __init__(self,b):self.b=b;self.pos=0
 def take(self,n):assert self.pos+n<=len(self.b);v=self.b[self.pos:self.pos+n];self.pos+=n;return v
 def u2(self):return int.from_bytes(self.take(2),'big')
 def u4(self):return int.from_bytes(self.take(4),'big')
def attributes(r,pool):
 out=[]
 for _ in range(r.u2()):
  name=pool[r.u2()];n=r.u4();data=r.take(n);d={'name':name,'bytes':n,'sha256':hashlib.sha256(data).hexdigest()}
  if name=='Code':
   rr=Reader(data);d['max_stack']=rr.u2();d['max_locals']=rr.u2();d['code_bytes']=rr.u4();rr.take(d['code_bytes']);rr.take(rr.u2()*8);d['attributes']=attributes(rr,pool);assert rr.pos==len(data)
  out.append(d)
 return out
with zipfile.ZipFile(io.BytesIO(b)) as z:
 assert z.testzip() is None
 for info in z.infolist():
  data=z.read(info);row={'name':info.filename,'bytes':len(data),'crc32':info.CRC,'sha256':hashlib.sha256(data).hexdigest()}
  if info.filename.endswith('.class'):
   r=Reader(data);assert r.u4()==0xcafebabe;minor=r.u2();major=r.u2();count=r.u2();pool=[None]*count;i=1
   while i<count:
    tag=r.take(1)[0]
    if tag==1:pool[i]=r.take(r.u2()).decode('utf8','replace')
    elif tag in [3,4,9,10,11,12,17,18]:r.take(4)
    elif tag in [5,6]:r.take(8);i+=1
    elif tag in [7,8,16,19,20]:r.take(2)
    elif tag==15:r.take(3)
    else:raise AssertionError(tag)
    i+=1
   row.update({'class_version':[major,minor],'constant_pool_count':count});r.take(6);r.take(2*r.u2());members={}
   for kind in ['fields','methods']:
    items=[]
    for _ in range(r.u2()):
     flags=r.u2();name=pool[r.u2()];descriptor=pool[r.u2()];attrs=attributes(r,pool);items.append({'name':name,'descriptor':descriptor,'flags':flags,'attributes':attrs})
    members[kind]=items
   row.update(members);row['attributes']=attributes(r,pool);assert r.pos==len(data);row['all_class_structure_bytes_consumed']=True
  rows.append(row)
report={'jar_sha256':hashlib.sha256(b).hexdigest(),'entries':rows,'classes':sum(x['name'].endswith('.class') for x in rows),'methods':sum(len(x.get('methods',[])) for x in rows),'fields':sum(len(x.get('fields',[])) for x in rows),'all_entry_crc_pass':True,'limits':'Byte-structural parse, attributes and code-region extents, not JVM verification or instruction-by-instruction semantic reverse engineering.'};(C/'jar-structure-v100.json').write_text(json.dumps(report,indent=2)+'\n')
print('classes',report['classes'],'methods',report['methods'],'fields',report['fields'])
for x in rows:
 if 'class_version' in x:print(x['name'],x['class_version'],'methods:',','.join(y['name']+y['descriptor'] for y in x['methods']))
