#!/usr/bin/env python3
import hashlib,json,re
from pathlib import Path
D=Path(__file__).resolve().parents[1]/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 p=D/'dedup-disc21-bank-diagnostic-native-20261011.json';r=json.loads(p.read_text());assert r['status']=='complete_bounded_disc21_bank_diagnostic' and r['returncode']==0
 pins=dict(r['pins']);assert all(sha(k)==v for k,v in pins.items());assert sha(r['log'])==r['log_sha256'];assert Path(r['log']).read_text()==r['stdout']+r['stderr']
 e=json.loads(r['stdout']);assert e==r['evidence'];assert e['attempts']==25 and e['managed_used']==0 and e['supported'] is False;assert e['raw_proposals']==e['selected_points']==786 and e['pixel_attempts']==4
 assert r['max_rss_bytes']==int(re.search(r'(\d+)\s+maximum resident set size',r['stderr'])[1])
 pilot=D/'dedup-disc21-pilot-native-20261011.json';a=json.loads(pilot.read_text());row=a['results'][0];assert row['status']=='inconclusive';assert row['command'][5:9]==r['command'][5:9]
 manifest=D/'dedup-disc21-bank-diagnostic-native-snapshot-20261011.json';m=json.loads(manifest.read_text());assert all(sha(Path(m['snapshot'])/k)==v for k,v in m['file_hashes'].items())
 pins.update({str(x):sha(x) for x in [p,pilot,Path(__file__),Path(r['log'])]});out=D/'dedup-disc21-bank-diagnostic-native-audit-20261011.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_disc21_positive_bank_diagnostic','pins':pins,'evidence':e,'conclusion':'25 banks yielded 786 points and four pixel calls. Unknown discards failed candidate transforms; zero final inliers does not prove no geometric candidate. Exact pixel refusal cause remains unqualified.'},indent=2)+'\n')
if __name__=='__main__':main()
