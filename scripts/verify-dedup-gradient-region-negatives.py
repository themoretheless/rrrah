#!/usr/bin/env python3
"""Audit one-query three-model regional negative measurements; no broad precision claim."""
import argparse,hashlib,json,math,struct
from pathlib import Path
p=argparse.ArgumentParser()
for key in ('manifest','probe','report','query','output'):p.add_argument(key)
p.add_argument('--checkpoint',action='store_true');p.add_argument('--bidirectional',action='store_true');p.add_argument('--six-regions',action='store_true');a=p.parse_args();assert not(a.bidirectional and a.six_regions)
raw=Path(a.report).read_bytes();manifest_raw=Path(a.manifest).read_bytes();r=json.loads(raw);m=json.loads(manifest_raw)
digest=lambda path:hashlib.sha256(Path(path).read_bytes()).hexdigest()
assert r['mode']==('six_regions_negative_control' if a.six_regions else 'five_bidirectional_regions_negative_control' if a.bidirectional else 'five_all_regions_negative_control') and r['query']==a.query
assert r['manifest_sha256']==hashlib.sha256(manifest_raw).hexdigest() and r['probe_sha256']==digest(a.probe)
query=next(v['right'] for v in m['positive_pairs'] if v['query_id']==a.query)
originals=[v for v in m['images']['original'] if v['group_id']!=query['group_id']]
assert len(originals)==r['required_pairs']==156 and 0<len(r['results'])<=156
assert r['status'] in ('checkpoint','complete')
if not a.checkpoint:assert len(r['results'])==156 and r['status']=='complete'
images={};whole=[];supported=[];retrieved=0
def expected_domains(matrix, source, target):
    """Reconstruct the declared uniform grid from image headers and native H."""
    if matrix is None:
        return []
    assert len(matrix) == 3 and all(len(row) == 3 for row in matrix)
    assert all(math.isfinite(v) for row in matrix for v in row)
    sw, sh = source
    tw, th = target
    result = []
    for row in range(4):
        for col in range(4):
            x, y = col * sw // 4, row * sh // 4
            width, height = (col + 1) * sw // 4 - x, (row + 1) * sh // 4 - y
            if not width or not height:
                continue
            mapped = []
            for cx, cy in ((x, y), (x + width - 1, y), (x, y + height - 1), (x + width - 1, y + height - 1)):
                q = [r[0] * cx + r[1] * cy + r[2] for r in matrix]
                assert all(math.isfinite(v) for v in q) and q[2] != 0
                mapped.append((q[0] / q[2], q[1] / q[2]))
            starts = [max(0, min(bound, math.floor(min(v[axis] for v in mapped)))) for axis, bound in enumerate((tw, th))]
            ends = [max(0, min(bound, math.ceil(max(v[axis] for v in mapped)) + 1)) for axis, bound in enumerate((tw, th))]
            if ends[0] <= starts[0] or ends[1] <= starts[1]:
                continue
            result.append([[x, y, width, height], [*starts, ends[0] - starts[0], ends[1] - starts[1]]])
    return result



def inverse(matrix):
 scale=max(abs(v) for row in matrix for v in row);assert math.isfinite(scale) and scale>0
 matrix=[[v/scale for v in row] for row in matrix]
 cofactors=[]
 for i in range(3):
  row=[]
  for j in range(3):
   minor=[[matrix[y][x] for x in range(3) if x!=j] for y in range(3) if y!=i]
   row.append((-1)**(i+j)*(minor[0][0]*minor[1][1]-minor[0][1]*minor[1][0]))
  cofactors.append(row)
 determinant=sum(matrix[0][j]*cofactors[0][j] for j in range(3));assert math.isfinite(determinant) and determinant!=0
 return [[cofactors[j][i] for j in range(3)] for i in range(3)]

def verify_regions(regions,model,dimensions,total,both=False):
 expected=expected_domains(model,*dimensions)
 if both and model is not None:expected += [pair[::-1] for pair in expected_domains(inverse(model),*dimensions[::-1])]
 assert [region['domains'] for region in regions]==expected
 actual=0
 for region in regions:
  assert type(region['accepted_region']) is bool
  areas=[]
  for domain,(width,height) in zip(region['domains'],dimensions):
   assert len(domain)==4 and all(type(v) is int and v>=0 for v in domain)
   x,y,dw,dh=domain;assert dw>0 and dh>0 and x+dw<=width and y+dh<=height;areas.append(dw*dh)
  if region['counts'] is None:assert not region['accepted_region'] and region['fit_failure']!='None'
  else:
   assert len(region['counts'])==2 and region['fit_failure']=='None';accepted=True
   for values,area in zip(region['counts'],areas):
    assert len(values)==3 and all(type(v) is int and v>=0 for v in values)
    matched,compared,source=values;assert matched<=compared<=source==area
    accepted &= compared>=1000 and compared>=source*.3 and matched>=compared*.9
   assert accepted==region['accepted_region']
  actual+=int(region['accepted_region'])
 assert type(total) is int and total==actual
 return actual
for row,original in zip(r['results'],originals):
 pair_id=query['filename']+'/'+original['filename'];assert row['query_id']==pair_id and row['label']=='different_publisher_origin' and row['returncode']==0
 e=row['evidence'];assert e['status']=='ok' and type(e['retrieved']) is bool and type(e['candidate']) is bool and type(e['managed_used']) is int and e['managed_used']==0
 for key,cap in (('managed_peak',64*1024*1024),('indexed_features',13000 if a.six_regions else 10000),('descriptor_hits',45_000_000 if a.six_regions else 34_000_000)):assert type(e[key]) is int and 0<=e[key]<=cap
 dims=[]
 for image in (original,query):
  path=Path(image['normalized_path']);assert digest(path)==image['normalized_sha256'];images[str(path)]=image['normalized_sha256'];header=path.read_bytes()[:24];assert header[:8]==b'\x89PNG\r\n\x1a\n' and header[12:16]==b'IHDR';dims.append(struct.unpack('>II',header[16:24]))
 count=0
 if e['retrieved']:
  retrieved+=1;flags=e['five_accepted_searches'];assert len(flags)==5 and all(type(v) is bool for v in flags) and e['candidate']==any(flags)
  count+=verify_regions(e['regions'],e['regional_geometry'],dims,e['region_support_count'])
  lanes=e['gradient_regions'];assert len(lanes)==2
  for index,lane in enumerate(lanes):
   model=e['gradient_geometry' if index==0 else 'interpolated_geometry'];assert (lane is None)==(model is None)
   if lane is not None:assert lane['geometry']==model;count+=verify_regions(lane['regions'],model,dims,lane['region_support_count'],a.bidirectional or a.six_regions)
 else:
  assert e['candidate'] is False and e['regions']==[] and type(e['region_support_count']) is int and e['region_support_count']==0
  assert not any(key in e for key in ('five_accepted_searches','four_accepted_searches','regional_geometry','regional_whole_candidate','gradient_regions','gradient_geometry','interpolated_geometry','gradient_counts','interpolated_counts'))
 spatial_whole=False
 if a.six_regions and e['retrieved']:
  assert e['spatial_grid']==[4,4,32]
  g=e['spatial_gradient'];assert type(g['candidate']) is bool
  for key in ('correspondences','inliers'):assert type(g[key]) is int and 0<=g[key]<=1500
  assert g['inliers']<=g['correspondences']
  expected_whole=False
  if g['pixels'] is not None and g['pixels']['status']=='ok':
   counts=g['pixels']['counts'];assert len(counts)==2
   for (matched,compared,source),(width,height) in zip(counts,dims):
    assert all(type(v) is int for v in (matched,compared,source)) and 0<=matched<=compared<=source==width*height
   expected_whole=all(n>=1000 and n>=s*.3 and v>=n*.9 for v,n,s in counts)
  assert g['candidate']==expected_whole;spatial_whole=g['candidate']
  lane=e['spatial_gradient_regions'];model=g['geometry'];assert (lane is None)==(model is None)
  if lane is not None:assert lane['geometry']==model;count+=verify_regions(lane['regions'],model,dims,lane['region_support_count'],True)
 if e['candidate'] or spatial_whole:whole.append(pair_id)
 if count:supported.append(pair_id)
assert r['image_hashes']==images
assert Path(a.report).read_bytes()==raw and Path(a.manifest).read_bytes()==manifest_raw and r['probe_sha256']==digest(a.probe),'Inputs changed; retry audit.'
out={'verified_pairs':len(r['results']),'required_pairs':156,'bidirectional_gradient_grids':a.bidirectional or a.six_regions,'six_regions':a.six_regions,'query':a.query,'retrieved_pairs':retrieved,'whole_positive_pairs':whole,'region_supported_pairs':supported,'verified_images':len(images),'report_sha256':hashlib.sha256(raw).hexdigest(),'scope':'One fixed recovered query against all different publisher-origin originals. Finite negative control, no semantic/burst, all-query precision or full library qualification.'}
Path(a.output).write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
