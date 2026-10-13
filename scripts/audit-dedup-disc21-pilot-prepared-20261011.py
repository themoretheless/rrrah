#!/usr/bin/env python3
"""Offline replay of retained HTTP ranges, ZIP CRCs, images and published labels."""
import csv,hashlib,io,json,zipfile
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
class Replay:
 def __init__(self,a):self.size=a['size'];self.ranges=a['ranges'];self.pos=0
 def seek(self,o,w=0):self.pos=o if w==0 else (self.pos+o if w==1 else self.size+o);return self.pos
 def tell(self):return self.pos
 def seekable(self):return True
 def read(self,n=-1):
  n=self.size-self.pos if n<0 else min(n,self.size-self.pos)
  if not n:return b''
  r=next(r for r in self.ranges if r['start']<=self.pos and r['end']>=self.pos+n-1);data=Path(r['path']).read_bytes();assert len(data)==r['end']-r['start']+1;offset=self.pos-r['start'];self.pos+=n;return data[offset:offset+n]
def main():
 p=D/'dedup-disc21-pilot-prepared-20261011.json';d=json.loads(p.read_text());assert d['status']=='prepared_published_label_pilot' and all(sha(k)==v for k,v in d['pins'].items());gt=next(Path(k) for k in d['pins'] if k.endswith('/dev_ground_truth.csv'));rows=list(csv.reader(io.StringIO(gt.read_text())));mapping=dict(rows);assert len(rows)==len(mapping)==10000;assert mapping[d['positive_query']]==d['reference'] and d['negative_query'] not in mapping
 assert d['plan']==[{'query':d['positive_query'],'reference':d['reference'],'published_label':'copy'},{'query':d['negative_query'],'reference':d['reference'],'published_label':'no_reference_match'}];assert len(d['images'])==3
 for a in d['archives']:
  assert a['url'].startswith('https://dl.fbaipublicfiles.com/image_similarity_challenge/public/') and a['etag']==a['head'].get('ETag',a['head'].get('Etag'))
  assert sum(r['end']-r['start']+1 for r in a['ranges'])==a['bytes_downloaded']<40*1024*1024
  with zipfile.ZipFile(Replay(a)) as z:
   for image in d['images']:
    if image['archive']!=a['url'].rsplit('/',1)[-1]:continue
    info=z.getinfo(image['member']);assert info.CRC==image['zip_crc32'] and info.file_size==image['size'];b=z.read(info);assert b==Path(image['path']).read_bytes() and hashlib.sha256(b).hexdigest()==image['sha256']
 out=D/'dedup-disc21-pilot-prepared-audit-20261011.json';assert not out.exists();pins=dict(d['pins']);pins.update({str(x):sha(x) for x in [p,Path(__file__)]});out.write_text(json.dumps({'status':'verified_disc21_published_labels_and_range_extraction','pins':pins,'plan':d['plan'],'downloaded_archive_bytes':sum(a['bytes_downloaded'] for a in d['archives']),'scope':'Offline replay verifies3 exact JPEG members/ZIP CRCs and hashes from20 retained HTTP ranges, two version/ETag-pinned official archives, and10000-row published ground truth. Dataset distractor semantics and annotation-error caveat apply. Not full DISC21 or semantic/burst precision qualification.'},indent=2)+'\n');print('Verified3 JPEG members, published positive/distractor labels and bounded archive range extraction')
if __name__=='__main__':main()
