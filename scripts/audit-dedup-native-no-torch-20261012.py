#!/usr/bin/env python3
"""Verify identical native evidence and report observed process memory."""
import hashlib,json,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 p=D/'dedup-native-no-torch-20261012.json';r=json.loads(p.read_text());assert r['status']=='complete_native_no_torch_control' and r['returncode']==0
 assert all(sha(k)==v for k,v in r['pins'].items());assert sha(r['build_log'])==r['build_log_sha256']
 ap=D/'dedup-native-bank-all-positives-prefix-0164-audit-20261011.json';a=json.loads(ap.read_text());assert all(sha(k)==v for k,v in a['pins'].items());capture=Path(a['captured_report']);base=json.loads(capture.read_text())['results'][163];old=json.loads(base['stdout']);new=json.loads(r['stdout'])
 for k in ['files','pairs','opposite_trials','managed_retained_before_drop','managed_used','managed_peak']:assert new[k]==old[k],k
 assert not new['pairs'][0]['model_attempted'] and new['pairs'][0]['support'] and new['managed_used']==0
 assert not any('torch' in x.lower() for x in r['dynamic_libraries'].splitlines()[1:])
 rss=int(re.search(r'(\d+)\s+maximum resident set size',r['stderr'])[1]);original=int(re.search(r'(\d+)\s+maximum resident set size',base['stderr'])[1]);out=D/'dedup-native-no-torch-audit-20261012.json';assert not out.exists()
 out.write_text(json.dumps({'status':'verified_identical_native_outcome_without_linked_torch','baseline_rss_bytes':original,'no_torch_rss_bytes':rss,'rss_difference_bytes':rss-original,'managed_peak_bytes':new['managed_peak'],'elapsed_seconds':r['elapsed_seconds'],'baseline_elapsed_seconds':base['elapsed_seconds'],'pins':{str(x):sha(x) for x in [p,ap,capture,Path(__file__)]},'scope':'Single identical frozen-input policy control on a concurrent host. Dynamic dependencies contain no Torch; provider invocation is fatal. Difference does not prove a universal allocation cause, speedup or process cap.'},indent=2)+'\n');print('Verified exact native evidence; RSS',original,'->',rss)
if __name__=='__main__':main()
