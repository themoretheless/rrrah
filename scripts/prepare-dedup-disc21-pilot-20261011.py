#!/usr/bin/env python3
"""Bounded HTTP-range extraction of published DISC21 labelled pilot images."""
import csv,hashlib,io,json,os,urllib.request,zipfile
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp');BASE='https://dl.fbaipublicfiles.com/image_similarity_challenge/public/'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
class Remote:
 def __init__(self,url,tag,root):
  self.url=url;self.tag=tag;self.root=root;self.pos=0;self.bytes=0;self.ranges=[]
  with urllib.request.urlopen(urllib.request.Request(url,method='HEAD'),timeout=30) as response:
   self.size=int(response.headers['Content-Length']);self.etag=response.headers['ETag'];self.head=dict(response.headers)
 def seek(self,offset,whence=0):
  self.pos=offset if whence==0 else (self.pos+offset if whence==1 else self.size+offset);assert 0<=self.pos<=self.size;return self.pos
 def tell(self):return self.pos
 def seekable(self):return True
 def read(self,n=-1):
  n=self.size-self.pos if n<0 else min(n,self.size-self.pos)
  if not n:return b''
  assert n<=16*1024*1024 and self.bytes+n<=40*1024*1024,'bounded archive access'
  start=self.pos;end=start+n-1;request=urllib.request.Request(self.url,headers={'Range':f'bytes={start}-{end}','If-Match':self.etag})
  with urllib.request.urlopen(request,timeout=45) as response:
   assert response.status==206 and response.headers['ETag']==self.etag
   assert response.headers['Content-Range']==f'bytes {start}-{end}/{self.size}'
   data=response.read(n+1);assert len(data)==n
  p=self.root/f'{self.tag}-range-{len(self.ranges):03d}.bin';p.write_bytes(data);self.ranges.append({'start':start,'end':end,'path':str(p),'sha256':sha(p)});self.bytes+=n;self.pos+=n;return data
 def evidence(self):return {'url':self.url,'size':self.size,'etag':self.etag,'head':self.head,'bytes_downloaded':self.bytes,'ranges':self.ranges}
def main():
 A=T/'rrrah-disc21-pilot-source-20261011';out=D/'dedup-disc21-pilot-prepared-20261011.json';assert not A.exists() and not out.exists();A.mkdir();gt=A/'dev_ground_truth.csv';gt.write_bytes((T/'rrrah-disc21-dev-ground-truth-20261011.csv').read_bytes());rows=list(csv.reader(io.StringIO(gt.read_text())));assert len(rows)==10000 and all(len(x)==2 for x in rows);mapping={q:r for q,r in rows};assert len(mapping)==10000
 q,ref=next((q,r) for q,r in rows if int(r[1:])<50000);negative=next(f'Q{i:05d}' for i in range(50000) if f'Q{i:05d}' not in mapping)
 archives=[];images=[]
 for filename,ids in [('dev_queries.zip',[q,negative]),('references_0.zip',[ref])]:
  remote=Remote(BASE+filename,filename[:-4],A)
  with zipfile.ZipFile(remote) as z:
   names={Path(x.filename).stem:x for x in z.infolist() if x.filename.endswith('.jpg')};assert len(names)==50000
   for image_id in ids:
    info=names[image_id];assert info.file_size<=5242880;data=z.read(info);p=A/(image_id+'.jpg');p.write_bytes(data);assert data[:2]==b'\xff\xd8';images.append({'id':image_id,'path':str(p),'sha256':sha(p),'archive':filename,'member':info.filename,'zip_crc32':info.CRC,'compressed_size':info.compress_size,'size':len(data)})
  archives.append(remote.evidence())
 pins={str(p):sha(p) for p in [gt,Path(__file__)]};pins.update({x['path']:x['sha256'] for x in images})
 for a in archives:pins.update({x['path']:x['sha256'] for x in a['ranges']})
 result={'status':'prepared_published_label_pilot','ground_truth_url':BASE+'dev_ground_truth.csv','dataset_url':'https://sites.google.com/view/isc2021/dataset','paper_url':'https://arxiv.org/html/2106.09672v2','published_license_link':'https://www.drivendata.org/competitions/79/competition-image-similarity-1-dev/page/381/','ground_truth_rows':len(rows),'positive_query':q,'reference':ref,'negative_query':negative,'images':images,'archives':archives,'pins':pins,'plan':[{'query':q,'reference':ref,'published_label':'copy'},{'query':negative,'reference':ref,'published_label':'no_reference_match'}],'scope':'Deterministic first positive in reference shard0 and first query absent from published10000-positive mapping. Official DISC21 distractor semantics; no manual semantic/burst audit, annotation infallibility or broad precision/recall qualification. Local research inputs, not vendored product assets.'};out.write_text(json.dumps(result,indent=2)+'\n');print(result['plan'],sum(a['bytes_downloaded'] for a in archives),flush=True)
if __name__=='__main__':main()
