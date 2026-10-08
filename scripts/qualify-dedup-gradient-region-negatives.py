#!/usr/bin/env python3
"""Measure one recovered query against all different-origin originals."""
import argparse,hashlib,json,subprocess
from pathlib import Path
p=argparse.ArgumentParser()
for key in ('manifest','probe','query','output'):p.add_argument(key)
p.add_argument('--bidirectional',action='store_true')
p.add_argument('--spatial-regions',action='store_true')
p.add_argument('--six-regions',action='store_true')
a=p.parse_args();assert sum((a.bidirectional,a.spatial_regions,a.six_regions))<=1;digest=lambda path:hashlib.sha256(Path(path).read_bytes()).hexdigest()
m=json.loads(Path(a.manifest).read_text());query=next(v['right'] for v in m['positive_pairs'] if v['query_id']==a.query)
originals=[v for v in m['images']['original'] if v['group_id']!=query['group_id']];assert len(originals)==156
pins={'manifest_sha256':digest(a.manifest),'probe_sha256':digest(a.probe)}
rows=[];images={};out=Path(a.output)
for original in originals:
 for image in (original,query):assert digest(image['normalized_path'])==image['normalized_sha256'];images[image['normalized_path']]=image['normalized_sha256']
 r=subprocess.run([a.probe,'--six-regions-collection-pair' if a.six_regions else '--spatial-gradient-interpolated-regional-collection-pair' if a.spatial_regions else '--five-bidirectional-regions-collection-pair' if a.bidirectional else '--five-all-regions-collection-pair',original['normalized_path'],query['normalized_path']],capture_output=True,text=True)
 row={'query_id':query['filename']+'/'+original['filename'],'label':'different_publisher_origin','returncode':r.returncode,'stderr':r.stderr}
 if r.returncode==0:row['evidence']=json.loads(r.stdout)
 rows.append(row)
 assert pins=={'manifest_sha256':digest(a.manifest),'probe_sha256':digest(a.probe)}
 assert all(digest(name)==value for name,value in images.items())
 out.write_text(json.dumps({'mode':'six_regions_negative_control' if a.six_regions else 'spatial_gradient_regions_negative_control' if a.spatial_regions else 'five_bidirectional_regions_negative_control' if a.bidirectional else 'five_all_regions_negative_control','status':'checkpoint' if r.returncode==0 else 'failed','query':a.query,'required_pairs':156,**pins,'image_hashes':images,'results':rows},indent=2)+'\n')
 assert r.returncode==0,(row['query_id'],r.stderr)
 e=row['evidence'];print(json.dumps({'pair':row['query_id'],'whole':e['candidate'],'binary_regions':e['region_support_count'],'gradient_regions':[None if lane is None else lane['region_support_count'] for lane in e.get('gradient_regions',[])]}),flush=True)
d=json.loads(out.read_text());d['status']='complete';out.write_text(json.dumps(d,indent=2)+'\n')
