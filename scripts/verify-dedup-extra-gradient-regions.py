#!/usr/bin/env python3
"""Audit all 58 previously unconfirmed pairs with native gradient geometry."""
import argparse,hashlib,json,math,struct
from pathlib import Path
p=argparse.ArgumentParser()
for key in ('manifest','probe','report','whole_reference','regional_reference','output'):p.add_argument(key,type=Path)
p.add_argument('--checkpoint',action='store_true');p.add_argument('--all-pairs',action='store_true');a=p.parse_args()
paths=(a.manifest,a.report,a.whole_reference,a.regional_reference)
raw={v:v.read_bytes() for v in paths};m,r,w,b=[json.loads(raw[v]) for v in paths]
def digest(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
assert len(m['positive_pairs'])==229 and len({v['query_id'] for v in m['positive_pairs']})==229
assert w['mode']=='complementary_gradient_portfolio' and b['mode']=='pyramid_region_grid'
for ref,exe in ((w,'photo-probe-five-lane'),(b,'photo-probe-native-region-grid')):
 assert ref['probe_sha256']==digest(a.probe.parent/exe)
 assert ref['manifest_sha256']==digest(a.manifest)
 assert ref['required_pairs']==ref['completed_pairs']==len(ref['results'])==229
 assert ref['status']=='complete_measurement_not_full_library_qualification'
 assert [row['query_id'] for row in ref['results']]==[pair['query_id'] for pair in m['positive_pairs']]
 assert all(row['status']=='ok' and row['returncode']==0 for row in ref['results'])
whole={v['query_id']:v['evidence'] for v in w['results']};binary={v['query_id']:v['evidence'] for v in b['results']}
selected=[pair for pair in m['positive_pairs'] if not whole[pair['query_id']]['candidate'] and binary[pair['query_id']]['region_support_count']==0 and any(whole[pair['query_id']][key] is not None for key in ('gradient_geometry','interpolated_geometry'))]
assert len(selected)==58
if a.all_pairs:selected=m['positive_pairs']
required=229 if a.all_pairs else 58
assert r['required_pairs']==required and 0<len(r['results'])<=required
assert r['status'] in ('checkpoint','complete')
if not a.checkpoint:assert r['status']=='complete' and len(r['results'])==required
assert [v['query_id'] for v in r['results']]==[v['query_id'] for v in selected[:len(r['results'])]]
assert r['input_hashes']=={str(v):digest(v) for v in (a.manifest,a.probe,a.whole_reference,a.regional_reference)}
images={};recovered=[];supports=[0,0];whole_count=0;region_count=0;combined_count=0
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


for row,pair in zip(r['results'],selected):
 qid=pair['query_id'];e=row['evidence'];dims=[]
 for side in ('left','right'):
  item=pair[side];path=Path(item['normalized_path']);assert digest(path)==item['normalized_sha256'];images[str(path)]=item['normalized_sha256']
  header=path.read_bytes()[:24];assert header[:8]==b'\x89PNG\r\n\x1a\n' and header[12:16]==b'IHDR';dims.append(struct.unpack('>II',header[16:24]))
 for key,value in whole[qid].items():
  if key!='managed_peak':assert e[key]==value,(qid,key)
 for key,other in (('regional_geometry','geometry'),('regional_whole_candidate','candidate'),('regions','regions'),('region_support_count','region_support_count')):assert e[key]==binary[qid][other]
 assert e['retrieved'] is True and type(e['managed_used']) is int and e['managed_used']==0
 for key,cap in (('managed_peak',64*1024*1024),('indexed_features',10000),('descriptor_hits',34_000_000)):
  assert type(e[key]) is int and 0<=e[key]<=cap
 lanes=e['gradient_regions'];assert len(lanes)==2;found=False
 for index,lane in enumerate(lanes):
  model=whole[qid]['gradient_geometry' if index==0 else 'interpolated_geometry'];assert (lane is None)==(model is None)
  if lane is None:continue
  assert lane['geometry']==model
  assert [v['domains'] for v in lane['regions']]==expected_domains(model,*dims)
  total=0
  for region in lane['regions']:
   assert type(region['accepted_region']) is bool
   areas=[]
   for domain,(width,height) in zip(region['domains'],dims):
    assert len(domain)==4 and all(type(v) is int and v>=0 for v in domain)
    x,y,dw,dh=domain;assert dw>0 and dh>0 and x+dw<=width and y+dh<=height;areas.append(dw*dh)
   if region['counts'] is None:assert not region['accepted_region'] and region['fit_failure']!='None'
   else:
    assert region['fit_failure']=='None' and len(region['counts'])==2
    accepted=True
    for values,area in zip(region['counts'],areas):
     assert len(values)==3 and all(type(v) is int and v>=0 for v in values)
     matched,compared,source=values;assert matched<=compared<=source==area
     accepted &= compared>=1000 and compared>=source*.3 and matched>=compared*.9
    assert region['accepted_region']==accepted
   total+=int(region['accepted_region'])
  assert type(lane['region_support_count']) is int and lane['region_support_count']==total
  supports[index]+=total;found |= total>0
 if found and not whole[qid]['candidate'] and binary[qid]['region_support_count']==0:recovered.append(qid)
 whole_count+=int(e['candidate']);region_count+=int(found or e['region_support_count']>0);combined_count+=int(e['candidate'] or found or e['region_support_count']>0)
assert r['image_hashes']==images
for path,data in raw.items():assert path.read_bytes()==data,'Snapshot changed; retry audit.'
assert r['input_hashes'][str(a.probe)]==digest(a.probe)
out={'verified_pairs':len(r['results']),'required_pairs':required,'whole_candidates':whole_count,'region_supported_pairs':region_count,'whole_or_region_supported_pairs':combined_count,'recovered_pairs':recovered,'gradient_region_supports':supports,'verified_images':len(images),'report_sha256':hashlib.sha256(raw[a.report]).hexdigest(),'input_hashes':r['input_hashes'],'scope':'Native two-source confirmations, declared manifest-order scope. All whole/binary regional fields match pinned constituents; gradient domains and pixel admission independently checked. No whole candidate promotion, precision claim, single386-image collection or full library qualification.'}
a.output.write_text(json.dumps(out,indent=2)+'\n');print(json.dumps({k:out[k] for k in ('verified_pairs','required_pairs','recovered_pairs','gradient_region_supports')},indent=2))
