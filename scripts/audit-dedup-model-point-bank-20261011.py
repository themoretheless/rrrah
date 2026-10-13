#!/usr/bin/env python3
import hashlib,json,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 p=D/'dedup-model-point-bank-audit-20261011.json';r=json.loads(p.read_text());assert r['status']=='complete_bounded_frozen_joint_model_tests' and r['tests']==93;mp=Path(r['source_manifest']);m=json.loads(mp.read_text());assert all(sha(Path(m['snapshot'])/k)==v for k,v in m['file_hashes'].items());pins=dict(r['pins']);assert all(sha(k)==v for k,v in pins.items())
 for row in r['results']:
  assert sha(row['binary'])==row['binary_sha256'] and sha(row['log'])==row['log_sha256'];assert re.findall(r'test result: ok\. (\d+) passed; 0 failed;',Path(row['log']).read_text())==[str(row['tests'])];pins[row['binary']]=row['binary_sha256'];pins[row['log']]=row['log_sha256']
 assert {x['suite']:x['tests'] for x in r['results']}=={'model_compare':19,'model_tiles':21,'search_portfolio':22,'portfolio_collection':23,'joint_witnesses':8}
 for f in ['crates/rrrah-dedup/src/model_compare.rs','crates/rrrah-dedup/tests/model_compare.rs','crates/rrrah-dedup/src/joint_witnesses.rs']:assert sha(R/f)==m['file_hashes'][f]
 pins.update({str(x):sha(x) for x in [p,mp,Path(__file__)]});out=D/'dedup-model-point-bank-verification-20261011.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_93_tests_guarded_model_point_bank_extraction','pins':pins,'scope':'Public mapped managed extraction with common complete source/weights/generation lifecycle. Crop coordinate mapping, last-clone release, source/weights mutation, empty bank and every caller cancellation checkpoint; existing tile/portfolio/collection and automatic bank tests. Not actual CPU LoFTR extraction or integrated multiwindow/file recovery/full corpus qualification.'},indent=2)+'\n');print('Verified93')
if __name__=='__main__':main()
