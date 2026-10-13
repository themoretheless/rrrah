#!/usr/bin/env python3
import hashlib,json,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 p=D/'dedup-model-bank-joint-recovery-audit-20261011.json';r=json.loads(p.read_text());assert r['status']=='complete_bounded_frozen_joint_model_tests' and r['tests']==134;mp=Path(r['source_manifest']);m=json.loads(mp.read_text());assert all(sha(Path(m['snapshot'])/k)==v for k,v in m['file_hashes'].items());pins=dict(r['pins']);assert all(sha(k)==v for k,v in pins.items())
 for row in r['results']:
  assert sha(row['binary'])==row['binary_sha256'] and sha(row['log'])==row['log_sha256'];assert re.findall(r'test result: ok\. (\d+) passed; 0 failed;',Path(row['log']).read_text())==[str(row['tests'])];pins[row['binary']]=row['binary_sha256'];pins[row['log']]=row['log_sha256']
 assert {x['suite']:x['tests'] for x in r['results']}=={'model_compare':26,'model_tiles':21,'search_portfolio':31,'portfolio_collection':26,'joint_witnesses':10,'geometry':9,'projective_domains':4,'joint_projective':4,'projective_pruning':3}
 for f in ['crates/rrrah-dedup/src/portfolio_collection.rs','crates/rrrah-dedup/tests/portfolio_collection.rs','crates/rrrah-dedup/src/model_banks.rs','crates/rrrah-dedup/src/joint_witnesses.rs','crates/rrrah-dedup/tests/joint_witnesses.rs','crates/rrrah-dedup/tests/model_compare.rs','crates/rrrah-dedup/src/search_portfolio.rs','crates/rrrah-dedup/tests/search_portfolio.rs']:assert sha(R/f)==m['file_hashes'][f]
 pins.update({str(x):sha(x) for x in [p,mp,Path(__file__)]});out=D/'dedup-model-bank-joint-recovery-verification-20261011.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_134_tests_model_bank_joint_recovery','pins':pins,'scope':'Bank primary and joint opposite-native recovery share original source/weights lifecycle. Complete aggregate/radius/projection/storage admission, primary positive laziness, learned+opposite unknown retained, both orders learned unknown to real native opposite fixture, final weights guard and last-bank fatal provider refusal. Pruning/geometry and legacy collection regressions included. Generic provider only; real CPU integrated recovery/full corpus/precision/all500/RSS qualification pending.'},indent=2)+'\n');print('Verified134')
if __name__=='__main__':main()
