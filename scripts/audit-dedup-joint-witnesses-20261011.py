#!/usr/bin/env python3
import hashlib,json,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 p=D/'dedup-joint-witnesses-domainfix-audit-20261011.json';r=json.loads(p.read_text());assert r['status']=='complete_bounded_frozen_joint_model_tests' and r['tests']==22;mp=Path(r['source_manifest']);m=json.loads(mp.read_text());assert all(sha(Path(m['snapshot'])/k)==v for k,v in m['file_hashes'].items());pins=dict(r['pins']);assert all(sha(k)==v for k,v in pins.items())
 for row in r['results']:
  assert sha(row['binary'])==row['binary_sha256'] and sha(row['log'])==row['log_sha256'];assert re.findall(r'test result: ok\. (\d+) passed; 0 failed;',Path(row['log']).read_text())==[str(row['tests'])];pins[row['binary']]=row['binary_sha256'];pins[row['log']]=row['log_sha256']
 assert {x['suite']:x['tests'] for x in r['results']}=={'geometry':9,'projective_domains':4,'joint_projective':4,'joint_witnesses':5}
 for f in ['crates/rrrah-dedup/src/joint_witnesses.rs','crates/rrrah-dedup/tests/joint_witnesses.rs','crates/rrrah-dedup/tests/fixtures/joint-witnesses-200301.bits.rs']:assert sha(R/f)==m['file_hashes'][f]
 pins.update({str(x):sha(x) for x in [p,mp,Path(__file__)]});out=D/'dedup-joint-witnesses-verification-20261011.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_22_tests_managed_joint_witness_aggregation','pins':pins,'scope':'Public bounded primitive plus legacy geometry regressions. Real669→10 exact witnesses, alias/residual filtering, preflight, malformed suffix, empty/zero domains, every cancellation checkpoint and last-clone release. First gate exposed zero-domain overflow; preserved failure, fixed child passed. Not integrated tile inference/portfolio/collection/full corpus qualification.'},indent=2)+'\n');print('Verified22')
if __name__=='__main__':main()
