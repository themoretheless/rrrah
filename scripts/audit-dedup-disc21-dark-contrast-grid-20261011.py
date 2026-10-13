#!/usr/bin/env python3
import hashlib,json,re
from pathlib import Path
D=Path(__file__).resolve().parents[1]/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 p=D/'dedup-disc21-dark-contrast-grid-20261011.json';r=json.loads(p.read_text());assert r['status']=='complete' and r['returncode']==r['build_returncode']==0
 pins=dict(r['pins']);assert all(sha(k)==v for k,v in pins.items());assert sha(r['build_log'])==r['build_log_sha256'];e=json.loads(r['stdout']);assert e['attempts']==25 and e['managed_used']==0 and e['raw_proposals']==e['selected_points']==1065 and e['supported'] is False
 rows=[]
 for line in r['stderr'].splitlines():
  if not line.startswith('GRID '):continue
  match=re.match(r'GRID bank=(\d+) contrast=([0-9.]+) rank_radius=(\d+) result=(.*)',line);assert match;bank,contrast,rank_radius,result=match.groups();counts=[tuple(map(int,x)) for x in re.findall(r'informative_pairs: (\d+), agreeing_pairs: (\d+)',result)];support='supported: true' in result
  if counts:
   assert len(counts)==2
   if support:assert all(n>=1000 and a/n>=0.9 for n,a in counts)
  else:assert result.startswith('Err(') and not support
  rows.append({'bank':int(bank),'contrast':float(contrast),'rank_radius':int(rank_radius),'counts':counts,'supported':support,'raw_result':result})
 banks={x['bank'] for x in rows};assert banks and len(rows)==10*len(banks);assert {(x['bank'],x['contrast'],x['rank_radius']) for x in rows}=={(b,c,a) for b in banks for c in [.005,.002,.001,.0005,.0001] for a in [2,8]}
 original=D/'dedup-disc21-bank-diagnostic-native-20261011.json';old=json.loads(original.read_text());assert all(sha(k)==v for k,v in old['pins'].items());pins.update(old['pins']);pins.update({str(x):sha(x) for x in [p,original,Path(__file__),Path(r['build_log'])]});out=D/'dedup-disc21-dark-contrast-grid-audit-20261011.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_exploratory_dark_contrast_grid','pins':pins,'original_evidence':e,'rows':rows,'supporting_contexts':[x for x in rows if x['supported']],'scope':'Fixed diagnostic contrast grid on dark screenshot, geometry and original terminal unchanged. Exploratory thresholds do not constitute a qualified recovery policy or negative precision evidence.'},indent=2)+'\n');print('calls',len(rows),'supporting',[(x['bank'],x['contrast'],x['rank_radius']) for x in rows if x['supported']])
if __name__=='__main__':main()
