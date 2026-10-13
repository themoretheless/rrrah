#!/usr/bin/env python3
import hashlib,json,re
from pathlib import Path
D=Path(__file__).resolve().parents[1]/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 p=D/'dedup-disc21-pixel-grid-20261011.json';r=json.loads(p.read_text());assert r['status']=='complete' and r['returncode']==r['build_returncode']==0
 pins=dict(r['pins']);assert all(sha(k)==v for k,v in pins.items());assert sha(r['build_log'])==r['build_log_sha256']
 baseline=D/'dedup-disc21-bank-diagnostic-native-20261011.json';b=json.loads(baseline.read_text());assert json.loads(r['stdout'])==json.loads(b['stdout']);assert r['command'][5:]==b['command'][5:]
 lines=[x for x in r['stderr'].splitlines() if x.startswith('GRID ')];assert len(lines)==56;rows=[]
 for line in lines:
  bank,filter_radius,rank_radius=map(int,re.match(r'GRID bank=(\d+) filter=(\d+) rank_radius=(\d+)',line).groups());assert 'result=Ok(' in line
  counts=[tuple(map(int,x)) for x in re.findall(r'informative_pairs: (\d+), agreeing_pairs: (\d+)',line)];assert len(counts)==2
  support='supported: true' in line;assert support==all(n>=1000 and a/n>=0.9 for n,a in counts)
  rows.append({'bank':bank,'filter_radius':filter_radius,'rank_radius':rank_radius,'counts':counts,'supported':support})
 assert {(x['bank'],x['filter_radius'],x['rank_radius']) for x in rows}=={(b,f,r) for b in [0,5] for f in [0,2,4,8,12,16,24] for r in [2,4,8,12]}
 chosen=[x for x in rows if x['filter_radius']==12 and x['rank_radius']==2];assert len(chosen)==2 and all(x['supported'] for x in chosen)
 pins.update({str(x):sha(x) for x in [p,baseline,Path(__file__),Path(r['build_log'])]});out=D/'dedup-disc21-pixel-grid-audit-20261011.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_fixed_pixel_grid_single_positive','pins':pins,'rows':rows,'candidate_policy':{'filter_radius':12,'rank_radius':2,'minimum_agreement':0.9,'minimum_pairs':1000},'scope':'Exploratory same-positive pixel-policy grid,56 calls. Rank-radius2/filter12 supports both fixed geometric candidates. Policy chosen after observing this positive; cohort and negative precision qualification required before adoption. Original terminal decision unchanged.'},indent=2)+'\n');print('verified56 grid calls; candidate rank2/filter12 supports both banks')
if __name__=='__main__':main()
