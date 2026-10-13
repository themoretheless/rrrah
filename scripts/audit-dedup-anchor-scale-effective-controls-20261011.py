#!/usr/bin/env python3
"""Merge only immutable audited rows, replacing every EXIF-affected raw domain."""
import hashlib,json
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def latest(kind):
 paths=list(D.glob(f'dedup-anchor-scale-{kind}-controls-prefix-*-audit-20261011.json'));assert paths
 p=max(paths,key=lambda p:int(p.name.split('prefix-')[1].split('-')[0]));a=json.loads(p.read_text());assert all(sha(k)==h for k,h in a['pins'].items());return p,a

def main():
 ip=D/'dedup-control-oriented-domains-audit-20261011.json';inventory=json.loads(ip.read_text());assert all(sha(p)==h for p,h in inventory['pins'].items());pins={str(ip):sha(ip),str(Path(__file__)):sha(__file__)};sources={};rows={}
 for kind in ['corpus','remapped','oriented']:
  p,a=latest(kind);pins[str(p)]=sha(p);sources[kind]=str(p);rows[kind]={(x['target'],x['filter_radius']):x for x in a['results']}
 effective=[];pending=[]
 for item in inventory['inventory']:
  kind='oriented' if item['changed_domain'] else 'corpus' if item['oriented_dimensions']==[2560,1920] else 'remapped'
  for radius in [8,12,16]:
   key=(item['target'],radius);row=rows[kind].get(key)
   if row is None:pending.append({'target':item['target'],'filter_radius':radius,'required_source':kind});continue
   if kind=='corpus':supported=row['supported'];e={'directions':row['directions']}
   else:
    e=row['anchor_result'];supported=e['supported'] if e and 'supported' in e else None
   effective.append({'target':item['target'],'filter_radius':radius,'source':kind,'oriented_dimensions':item['oriented_dimensions'],'supported':supported,'outcome':'pixel_evidence' if supported is not None else 'inconclusive','anchor_result':e})
 assert len(effective)+len(pending)==157*3;assert len({(x['target'],x['filter_radius']) for x in effective})==len(effective);out=D/f'dedup-anchor-scale-effective-prefix-{len(effective):04d}-audit-20261011.json';assert not out.exists();assert all(sha(p)==h for p,h in pins.items());out.write_text(json.dumps({'status':'verified_effective_scale_control_prefix','pins':pins,'sources':sources,'effective_checks':len(effective),'planned_checks':471,'pending_checks':len(pending),'results':effective,'pending':pending,'scope':'Audited original-domain stress controls over all157 inventory entries, with ten EXIF-affected raw inputs superseded exclusively by corrected bounded-decode verified rows. Fixed/synthetic geometry, no actual model search or independent semantic negatives; no precision, full corpus or API completion.'},indent=2)+'\n');print(len(effective),'effective;',len(pending),'pending;',sum(x['supported'] is True for x in effective),'supports;',sum(x['supported'] is None for x in effective),'inconclusive')
if __name__=='__main__':main()
