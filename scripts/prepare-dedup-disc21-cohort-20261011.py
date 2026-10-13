#!/usr/bin/env python3
"""Precommit a deterministic 8-positive/16-distractor DISC21 cohort."""
import csv,hashlib,importlib.util,io,json,zipfile
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 parent=D/'dedup-disc21-pilot-prepared-20261011.json';base=json.loads(parent.read_text());assert all(sha(k)==v for k,v in base['pins'].items());script=R/'scripts/prepare-dedup-disc21-pilot-20261011.py';assert sha(script)==base['pins'][str(script)];spec=importlib.util.spec_from_file_location('disc21_remote',script);module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
 class Cached(module.Remote):
  def read(self,n=-1):
   size=self.size-self.pos if n<0 else min(n,self.size-self.pos)
   if not size:return b''
   for r in self.ranges:
    if r['start']<=self.pos and r['end']>=self.pos+size-1:
     b=Path(r['path']).read_bytes();offset=self.pos-r['start'];self.pos+=size;return b[offset:offset+size]
   return super().read(n)
 out=D/'dedup-disc21-cohort-prepared-20261011.json';A=T/'rrrah-disc21-cohort-source-20261011';assert not out.exists() and not A.exists();A.mkdir();gt=next(Path(p) for p in base['pins'] if p.endswith('/dev_ground_truth.csv'));rows=list(csv.reader(io.StringIO(gt.read_text())));mapping=dict(rows);assert len(mapping)==len(rows)==10000;positive=[];seen=set()
 for q,r in rows:
  if int(r[1:])<50000 and r not in seen:positive.append((q,r));seen.add(r)
  if len(positive)==8:break
 assert len(positive)==8;negative=[f'Q{i:05d}' for i in range(50000) if f'Q{i:05d}' not in mapping][:16];assert len(negative)==16
 plan=[{'query':q,'reference':r,'published_label':'copy'} for q,r in positive]+[{'query':q,'reference':positive[i%8][1],'published_label':'no_reference_match'} for i,q in enumerate(negative)]
 needed={'dev_queries.zip':sorted({x['query'] for x in plan}),'references_0.zip':sorted({x['reference'] for x in plan})};old_images={x['id']:x for x in base['images']};images=[];archives=[]
 for filename,ids in needed.items():
  prior=next(a for a in base['archives'] if a['url'].endswith('/'+filename));remote=Cached(prior['url'],filename[:-4],A);assert remote.etag==prior['etag'] and remote.size==prior['size'];remote.ranges=list(prior['ranges']);remote.bytes=sum(r['end']-r['start']+1 for r in remote.ranges)
  with zipfile.ZipFile(remote) as z:
   members={Path(x.filename).stem:x for x in z.infolist() if x.filename.endswith('.jpg')};assert len(members)==50000
   for image_id in ids:
    if image_id in old_images:images.append(old_images[image_id]);continue
    info=members[image_id];assert info.file_size<=5242880;data=z.read(info);assert data[:2]==b'\xff\xd8';p=A/(image_id+'.jpg');p.write_bytes(data);images.append({'id':image_id,'path':str(p),'sha256':sha(p),'archive':filename,'member':info.filename,'zip_crc32':info.CRC,'compressed_size':info.compress_size,'size':len(data)})
  archives.append(remote.evidence())
 assert len(images)==32 and len({x['id'] for x in images})==32;pins={str(p):sha(p) for p in [parent,gt,script,Path(__file__)]};pins.update({x['path']:x['sha256'] for x in images})
 for a in archives:pins.update({r['path']:r['sha256'] for r in a['ranges']})
 out.write_text(json.dumps({'status':'prepared_fixed_published_label_cohort','parent':str(parent),'selection':'First8 distinct-reference positives in shard0 by published ground-truth row order; first16 query IDs absent from positive mapping; negative reference assigned round-robin across those8 sources. Selection independent of matching outcomes.','ground_truth_rows':10000,'positive_pairs':8,'negative_pairs':16,'plan':plan,'images':images,'archives':archives,'pins':pins,'scope':'Published DISC21 label cohort prepared before execution, original JPEGs only. Reuses pinned pilot ZIP ranges; CRC/ETag verified. Not runtime qualification, full DISC21, hard semantic or burst precision; publisher annotation errors remain possible.'},indent=2)+'\n');print('Prepared',len(plan),'pairs',len(images),'images; total cached+new archive bytes',sum(a['bytes_downloaded'] for a in archives),flush=True)
if __name__=='__main__':main()
