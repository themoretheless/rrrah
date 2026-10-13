#!/usr/bin/env python3
import hashlib,json,re
from pathlib import Path
D=Path(__file__).resolve().parents[1]/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 p=D/'dedup-disc21-candidate-trace-20261011.json';r=json.loads(p.read_text());assert r['status']=='complete' and r['returncode']==r['build_returncode']==0
 pins=dict(r['pins']);assert all(sha(k)==v for k,v in pins.items());assert sha(r['build_log'])==r['build_log_sha256']
 baseline=D/'dedup-disc21-bank-diagnostic-native-20261011.json';b=json.loads(baseline.read_text());assert json.loads(r['stdout'])==json.loads(b['stdout']);assert r['command'][5:]==b['command'][5:]
 lines=[x for x in r['stderr'].splitlines() if x.startswith('TRACE ')];assert len(lines)==6
 assert 'bank=0 witnesses=12' in lines[0] and 'bank=5 witnesses=14' in lines[3]
 rows=[]
 for bank,index in [(0,1),(5,4)]:
  pairs=re.findall(r'informative_pairs: (\d+), agreeing_pairs: (\d+)',lines[index]);assert len(pairs)==2
  fractions=[int(a)/int(n) for n,a in pairs];assert any(v<0.9 for v in fractions);assert 'supported: false' in lines[index] and 'radius=6 insufficient_geometry' in lines[index+1]
  rows.append({'bank':bank,'forward_agreement':fractions[0],'reverse_agreement':fractions[1],'radius2_supported':False,'radius6':'insufficient_geometry'})
 assert rows[0]['forward_agreement']>=0.9
 pins.update({str(x):sha(x) for x in [p,baseline,Path(__file__),Path(r['build_log'])]});out=D/'dedup-disc21-candidate-trace-audit-20261011.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_diagnostic_trace_same_terminal_decision','pins':pins,'candidates':rows,'scope':'Two candidate hypotheses fail original-pixel policy. Radius2 fails agreement; radius6 fails anchor geometry. Does not establish whether geometry error or scale-dependent filtering is dominant; no threshold change or recovery claim.'},indent=2)+'\n');print(rows)
if __name__=='__main__':main()
