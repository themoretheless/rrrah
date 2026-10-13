#!/usr/bin/env python3
import hashlib,json
from pathlib import Path
D=Path(__file__).resolve().parents[1]/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 p=D/'dedup-disc21-rank2-candidate-20261011.json';r=json.loads(p.read_text());assert r['status']=='complete' and r['returncode']==r['build_returncode']==0
 pins=dict(r['pins']);assert all(sha(k)==v for k,v in pins.items());assert sha(r['build_log'])==r['build_log_sha256']
 parent=D/'dedup-disc21-bank-diagnostic-native-snapshot-20261011.json';m=json.loads(parent.read_text());child=Path(next(k for k in pins if k.endswith('/crates/rrrah-dedup/src/model_banks.rs'))).parents[3]
 assert all(sha(child/f)==v for f,v in m['file_hashes'].items())
 old=Path('/Users/themoretheless/.codex/tmp/rrrah-disc21-bank-diagnostic-native-harness-20261011/src/bin/disc21_bank_diagnostic_files.rs');new=Path(next(k for k in pins if k.endswith('/src/bin/disc21_rank2_candidate_files.rs')));assert new.read_text()==old.read_text().replace('radius: 8,','radius: 2,')
 baseline=D/'dedup-disc21-bank-diagnostic-native-20261011.json';b=json.loads(baseline.read_text());assert r['command'][5:]==b['command'][5:];v=json.loads(r['stdout']);assert v['supported'] is True and v['attempts']==25 and v['managed_used']==0 and v['raw_proposals']==v['selected_points']==786 and v['geometry_inliers']>=10
 pins.update({str(x):sha(x) for x in [p,parent,baseline,old,Path(__file__),Path(r['build_log'])]});out=D/'dedup-disc21-rank2-candidate-audit-20261011.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_actual_cpu_single_positive_rank2_candidate','pins':pins,'evidence':v,'scope':'Unchanged production sources; single caller policy rank radius8→2. Real automatic model-bank search now supports published positive. Chosen after exploratory grid; no cohort/negative precision or general recovery qualification.'},indent=2)+'\n');print(v)
if __name__=='__main__':main()
