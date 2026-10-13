#!/usr/bin/env python3
import csv,hashlib,importlib.util,io,json,zipfile
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 p=D/'dedup-disc21-cohort-prepared-20261011.json';d=json.loads(p.read_text());assert d['status']=='prepared_fixed_published_label_cohort' and all(sha(k)==v for k,v in d['pins'].items());pa=D/'dedup-disc21-pilot-prepared-audit-20261011.json';old=json.loads(pa.read_text());assert all(sha(k)==v for k,v in old['pins'].items());script=R/'scripts/audit-dedup-disc21-pilot-prepared-20261011.py';assert sha(script)==old['pins'][str(script)];spec=importlib.util.spec_from_file_location('disc21_replay',script);module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
 gt=next(Path(k) for k in d['pins'] if k.endswith('/dev_ground_truth.csv'));rows=list(csv.reader(io.StringIO(gt.read_text())));mapping=dict(rows);assert len(rows)==len(mapping)==10000
 positives=[];seen=set()
 for q,r in rows:
  if int(r[1:])<50000 and r not in seen:positives.append((q,r));seen.add(r)
  if len(positives)==8:break
 negatives=[f'Q{i:05d}' for i in range(50000) if f'Q{i:05d}' not in mapping][:16];expected=[{'query':q,'reference':r,'published_label':'copy'} for q,r in positives]+[{'query':q,'reference':positives[i%8][1],'published_label':'no_reference_match'} for i,q in enumerate(negatives)];assert d['plan']==expected and d['positive_pairs']==8 and d['negative_pairs']==16
 ids={x['query'] for x in expected}|{x['reference'] for x in expected};assert len(d['images'])==len(ids)==32 and {x['id'] for x in d['images']}==ids
 for a in d['archives']:
  assert sum(x['end']-x['start']+1 for x in a['ranges'])==a['bytes_downloaded']<40*1024*1024
  with zipfile.ZipFile(module.Replay(a)) as z:
   for image in d['images']:
    if image['archive']!=a['url'].rsplit('/',1)[-1]:continue
    info=z.getinfo(image['member']);assert info.CRC==image['zip_crc32'] and info.file_size==image['size'];b=z.read(info);assert b==Path(image['path']).read_bytes() and hashlib.sha256(b).hexdigest()==image['sha256']
 pins=dict(d['pins']);pins.update({str(x):sha(x) for x in [p,pa,script,Path(__file__)]});out=D/'dedup-disc21-cohort-prepared-audit-20261011.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_fixed_disc21_8_positive_16_distractor_cohort','plan':expected,'pins':pins,'jpeg_images':32,'archive_bytes_cached_and_new':sum(a['bytes_downloaded'] for a in d['archives']),'scope':'Verifies deterministic precommitted published-label selection and exact original32 JPEG members via cached HTTP-range replay/ZIP CRCs. Preparation only, no runtime success or full DISC21/semantic/burst precision qualification.'},indent=2)+'\n');print('Verified32 JPEGs and deterministic8-positive/16-distractor plan')
if __name__=='__main__':main()
