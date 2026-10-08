"""Independent membership and geometric tap-count audit; no color oracle."""
import argparse,hashlib,json,math
from pathlib import Path
from dedup_jpeg_domain import oriented_dimensions
parser=argparse.ArgumentParser();parser.add_argument('report',type=Path);parser.add_argument('output',type=Path);args=parser.parse_args();assert not args.output.exists()
digest=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
r=json.loads(args.report.read_text());assert r['status']=='complete_native_rank_diagnostic' and r['required_cases']==len(r['rows'])==12
assert all(digest(k)==v for k,v in r['input_hashes'].items())
base=args.report.parent;root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');prepared=root/'prepared.json';manifest=json.loads(prepared.read_text())
positive=json.loads((base/'dedup-folded-training-diagnostic.json').read_text());negative_path=base/'dedup-affine-grid-meanfix-checkpoint-111.json';negative=json.loads(negative_path.read_text());audit=json.loads((base/'dedup-affine-grid-meanfix-checkpoint-111-audit.json').read_text());assert audit['report_sha256']==digest(negative_path) and audit['region_supports']==5
expected=[]
for row in positive['rows']:
 source=root/'original-resolution-negative-200201'/('200500.jpg' if row['case']=='screen_control' else '201700.jpg');query=root/'original-resolution-strong-all'/('200501.jpg' if row['case']=='screen_control' else '201702.jpg');values=list(map(float,Path(row['input']).read_text().split()));expected.append((row['case'],source,query,values))
for row in negative['rows']:
 if not row.get('supports'):continue
 matrix=list(map(float,Path(row['input']).read_text().split()));assert len(matrix)==9
 for index,region in enumerate(row['evidence']['regions']):
  if all('error' not in d and d['matched']>=.9*d['samples'] for d in region['directions']):expected.append((f'negative_{Path(row["source"]).stem}_{index}',Path(row['source']),root/'original-resolution-strong-all/200501.jpg',matrix+region['domains'][0]+region['domains'][1]))
assert len(expected)==12
def inverse(h):
 scale=max(abs(v) for axis in h for v in axis);assert scale>0;m=[[v/scale for v in axis] for axis in h];a,b,c=m[0];d,e,f=m[1];g,j,k=m[2]
 return [[e*k-f*j,c*j-b*k,b*f-c*e],[f*g-d*k,a*k-c*g,c*d-a*f],[d*j-e*g,b*g-a*j,a*e-b*d]]
def project(h,x,y):
 denominator=h[2][0]*x+h[2][1]*y+h[2][2];assert math.isfinite(denominator) and denominator!=0
 return [(h[i][0]*x+h[i][1]*y+h[i][2])/denominator for i in range(2)]
def counts(mapping,domain,source_domain,target_size,source_size):
 x,y,w,h=domain;ox,oy,ow,oh=source_domain;valid=reads=0
 offsets=[(0,0),(-8,-8),(0,-8),(8,-8),(-8,0),(8,0),(-8,8),(0,8),(8,8)]
 for py in range(y,y+h):
  for px in range(x,x+w):
   center=project(mapping,px,py)
   if not(ox<=center[0]<ox+ow and oy<=center[1]<oy+oh):continue
   complete=True
   for dx,dy in offsets:
    qx,qy=px+dx,py+dy
    if not(0<=qx<target_size[0] and 0<=qy<target_size[1]):complete=False;break
    reads+=5;tap=project(mapping,qx,qy)
    if not(0<=tap[0] and tap[0]+1<source_size[0] and 0<=tap[1] and tap[1]+1<source_size[1]):complete=False;break
   valid+=complete
 return valid,reads
summary=[]
for row,(name,source,query,values) in zip(r['rows'],expected):
 assert row['case']==name and row['source']==str(source) and row['query']==str(query)
 assert list(map(float,Path(row['input']).read_text().split()))==values and row['returncode']==0
 dims=[]
 for path,split in [(source,'original'),(query,'strong')]:
  item=next(v for v in manifest['images'][split] if v['filename']==path.name);assert digest(path)==item['source_sha256'];dims.append(oriented_dimensions(path,item['source_size']))
 h=[values[i:i+3] for i in range(0,9,3)];inv=inverse(h);domains=[list(map(int,values[9:13])),list(map(int,values[13:17]))]
 evidence=row['evidence'];assert evidence==json.loads(row['stdout']) and evidence['managed_used']==0 and 0<=evidence['managed_peak']<=512*1024*1024 and len(evidence['directions'])==2
 fractions=[]
 for index,(direction,domain,opposite,mapping) in enumerate(zip(evidence['directions'],domains[::-1],domains,[inv,inverse(inv)])):
  target_size,source_size=dims[1-index],dims[index];assert direction['status']=='ok'
  for d,size in [(domain,target_size),(opposite,source_size)]:assert len(d)==4 and all(type(v)is int and v>=0 for v in d) and 0<d[2]<=size[0]-d[0] and 0<d[3]<=size[1]-d[1]
  valid,reads=counts(mapping,domain,opposite,target_size,source_size)
  assert direction['sites']==domain[2]*domain[3]<=100000
  assert direction['valid_sites']==valid and direction['pixel_reads']==reads<=direction['sites']*45
  assert all(type(direction[k]) is int for k in ['informative_pairs','agreeing_pairs','sites','valid_sites','pixel_reads'])
  assert 1000<=direction['informative_pairs']<=8*valid and 0<=direction['agreeing_pairs']<=direction['informative_pairs']
  fractions.append(direction['agreeing_pairs']/direction['informative_pairs'])
 summary.append({'case':name,'fractions':fractions,'minimum_fraction':min(fractions)})
positive_min=min(v['minimum_fraction'] for v in summary[:7]);negative_max=max(v['minimum_fraction'] for v in summary[7:])
assert all(digest(k)==v for k,v in r['input_hashes'].items())
args.output.write_text(json.dumps({'status':'verified_rank_real_membership_and_taps','cases':12,'directions':24,'summary':summary,'minimum_positive_bidirectional_fraction':positive_min,'maximum_negative_bidirectional_fraction':negative_max,'single_threshold_separates_all_cases':positive_min>negative_max,'input_hashes':{str(p):digest(p) for p in [args.report,Path(__file__),Path(__file__).with_name('dedup_jpeg_domain.py'),prepared]},'scope':'Exact source/case/model/domain membership, source corpus digests, oriented dimensions and independently enumerated valid-site/read counts; ordinal contrast/agreement counts only bounded arithmetically. No luminance resampling or rank oracle, automatic classifier or broad precision.'},indent=2)+'\n')
