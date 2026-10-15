"""Read-only annotated contact sheets and native original hash/index corpus diagnostics."""
from pathlib import Path
import ast,hashlib,json,sys,math,os
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';W=R/'.dedup-work/near41-20261015';S=W/'source';sys.path.insert(0,str(W/'deps'))
import numpy as np,pandas as pd,imagehash
from PIL import Image,ImageDraw,ImageOps
from sklearn.neighbors import KDTree
from natsort import natsorted
from tqdm import tqdm
rows=json.loads((D/'near41-full-file-pins-20261015.json').read_text());images=[x for x in rows if x['path'].startswith('datasets/') and x.get('binary')];assert len(images)==540
ns={'np':np,'pd':pd,'os':os,'Image':Image,'KDTree':KDTree,'natsorted':natsorted,'tqdm':tqdm,'hash_algo_dict':{'phash':imagehash.phash}}
for file,name in [('duplicatefinder/NearDuplicateImageFinder.py','NearDuplicateImageFinder'),('duplicatefinder/KDTreeFinder.py','KDTreeFinder'),('dataset/ImageToHash.py','ImageToHash')]:
 t=ast.parse((S/'src/deduplication'/file).read_text());n=next(n for n in t.body if isinstance(n,ast.ClassDef) and n.name==name);exec(compile(ast.Module(body=[n],type_ignores=[]),file,'exec'),ns)
unique=[];by_blob={};records=[]
for row in images:
 p=S/row['path'];assert hashlib.sha256(p.read_bytes()).hexdigest()==row['sha256']
 with Image.open(p) as image:
  pixels=image.convert('RGB');pixel_sha=hashlib.sha256(pixels.tobytes()).hexdigest();size=image.size
 h=ns['ImageToHash'].img_hash(str(p),hash_algo='phash');digits=list(str(h))
 rec={'path':row['path'],'git_blob':row['sha'],'sha256':row['sha256'],'size':size,'rgb_pixel_sha256':pixel_sha,'phash':str(h),'hash_integer':int(str(h),16)}
 if row['sha'] not in by_blob:by_blob[row['sha']]=len(unique);unique.append(rec)
 rec['unique_visual_id']=by_blob[row['sha']];records.append(rec)
assert len(unique)==502
views=W/'visual-review';views.mkdir(exist_ok=False);sheets=[]
for first in range(0,len(unique),40):
 subset=unique[first:first+40];sheet=Image.new('RGB',(8*180,5*202),'#dddddd');draw=ImageDraw.Draw(sheet)
 for slot,rec in enumerate(subset):
  x=(slot%8)*180;y=(slot//8)*202
  with Image.open(S/rec['path']) as original:thumb=ImageOps.contain(original.convert('RGB'),(176,176));sheet.paste(thumb,(x+(180-thumb.width)//2,y+(178-thumb.height)//2))
  draw.text((x+2,y+180),f"{first+slot:03}: {Path(rec['path']).parent.name[:12]}",fill='black');draw.text((x+2,y+190),Path(rec['path']).name[:23],fill='black')
 p=views/f'sheet-{len(sheets):02}.png';sheet.save(p);sheets.append({'path':str(p),'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'visual_ids':list(range(first,first+len(subset)))})
dfrows=[]
for i,rec in enumerate(records):
 digits=list(rec['phash']);dfrows.append(dict(file=rec['path'],short_file=Path(rec['path']).name,hash=rec['phash'],hash_list=digits,**{str(k):int(v,16) for k,v in enumerate(digits)}))
df=pd.DataFrame(dfrows);f=ns['KDTreeFinder'](df,'manhattan');dist,indices=f._find_all(5,40);assert dist.shape==indices.shape==(540,5)
bit_radius=8;radius_edges=hit_edges=0;worst=[]
for i,rec in enumerate(records):
 truth={j for j,r in enumerate(records) if i!=j and (rec['hash_integer']^r['hash_integer']).bit_count()<=bit_radius}
 retrieved={int(j) for j,d in zip(indices[i],dist[i]) if int(j)!=i and d<=40}
 hits=truth&retrieved;radius_edges+=len(truth);hit_edges+=len(hits)
 if truth-retrieved:worst.append({'query':rec['path'],'within_hamming8':len(truth),'retrieved_hamming8':len(hits),'example_missing':records[min(truth-retrieved)]['path']})
out=D/'near41-visual-corpus-prepared-20261015.json';assert not out.exists();out.write_text(json.dumps({'status':'prepared_images_not_yet_viewed','images':540,'unique_visual_images':502,'identical_blob_aliases':38,'records':records,'sheets':sheets,'native_corpus_diagnostic':{'hash':'original ImageToHash.img_hash -> real ImageHashphash,64bits','index':'original KDTreeFinder wholeclass -> real sklearnKDTree','dataset_adapter':'ExplicitDataFrameconstruction fromoriginalimg_hash; originalsequentialbuilder modernPandasappendfailure notpatched','query_k':5,'numeric_manhattan_threshold':40,'comparison_hamming_radius':8,'directed_hamming_radius_edges':radius_edges,'retrieved_directed_hamming_radius_edges':hit_edges,'queries_with_omissions':len(worst),'examples':worst[:10],'scope':'Algorithmic candidate recall relative explicitHammingradius, notimagegroundtruth/visualduplicates or fair units-equivalent metric comparison. No performance or originalupstreamsuite claim.'},'limits':['Contactsheet preparation not visualreview; every sheet stillmustbeviewed','Sourceimages unchanged; thumbnails are analytical views only, noteditedsourceassets','No corpusdeletion/export/userdata/productionadoption']},indent=2)+'\n');print(json.dumps({'sheets':len(sheets),'images':540,'unique':502,'radius_edges':radius_edges,'retrieved':hit_edges,'queries_with_omissions':len(worst)}))
