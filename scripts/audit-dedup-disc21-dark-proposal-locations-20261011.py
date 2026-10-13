#!/usr/bin/env python3
import hashlib,json,re,sys
from pathlib import Path
D=Path(__file__).resolve().parents[1]/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def points(line):return [tuple(map(float,x)) for x in re.findall(r'source: \[([^,]+), ([^\]]+)\], target: \[([^,]+), ([^\]]+)\]',line)]
def main():
 kind=sys.argv[1];assert kind in ['raw','normalized'];name='dark-proposal-locations' if kind=='raw' else 'dark-normalized-proposals';p=D/f'dedup-disc21-{name}-20261011.json';r=json.loads(p.read_text());assert r['status']=='complete' and r['returncode']==r['build_returncode']==0
 pins=dict(r['pins']);assert all(sha(k)==v for k,v in pins.items());assert sha(r['build_log'])==r['build_log_sha256'];e=json.loads(r['stdout']);assert e['attempts']==25 and e['managed_used']==0
 banks={}
 for line in r['stderr'].splitlines():
  if line.startswith('BANK '):
   index=int(re.match(r'BANK bank=(\d+)',line)[1]);assert index not in banks;banks[index]=points(line)
 assert set(banks)==set(range(25));assert sum(map(len,banks.values()))==e['selected_points'];roi=[199,694,373,867];rows=[{'bank':i,'selected_points':len(ps),'source_points_in_manual_copy_tile':sum(roi[0]<=q[0]<roi[2] and roi[1]<=q[1]<roi[3] for q in ps)} for i,ps in sorted(banks.items())]
 if kind=='raw':
  assert e['raw_proposals']==e['selected_points']==1065
  prior=D/'dedup-disc21-dark-contrast-grid-20261011.json';old=json.loads(prior.read_text());pool=set(q for ps in banks.values() for q in ps);witnesses=[q for line in old['stderr'].splitlines() if line.startswith('WITNESSES ') for q in points(line)];assert len(witnesses)==21 and all(q in pool for q in witnesses);pins[str(prior)]=sha(prior)
 original=D/'dedup-disc21-bank-diagnostic-native-20261011.json';old=json.loads(original.read_text());assert all(sha(k)==v for k,v in old['pins'].items());pins.update(old['pins']);pins.update({str(x):sha(x) for x in [p,original,Path(__file__),Path(r['build_log'])]});out=D/f'dedup-disc21-{name}-audit-20261011.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_extraction_only_point_locations','kind':kind,'pins':pins,'selected_points':e['selected_points'],'source_points_in_manual_copy_tile':sum(x['source_points_in_manual_copy_tile'] for x in rows),'banks':rows,'manual_copy_tile_bbox_xyxy':roi,'scope':'Manual approximate tile annotation used only for offline localization analysis, never model input/search ROI. Geometry reduced to1 trial, terminal support is not qualification evidence. In-tile points are not verified correspondences; duplicates across banks counted.'},indent=2)+'\n');print(kind,e['selected_points'],sum(x['source_points_in_manual_copy_tile'] for x in rows))
if __name__=='__main__':main()
