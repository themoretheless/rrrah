#!/usr/bin/env python3
import hashlib,json,re
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 p=D/'dedup-model-bank-pixel-context-guards-audit-20261011.json';r=json.loads(p.read_text());assert r['status']=='complete_bounded_frozen_joint_model_tests' and r['tests']==152;mp=Path(r['source_manifest']);m=json.loads(mp.read_text());assert all(sha(Path(m['snapshot'])/k)==v for k,v in m['file_hashes'].items());pins=dict(r['pins']);assert all(sha(k)==v for k,v in pins.items())
 for row in r['results']:
  assert sha(row['binary'])==row['binary_sha256'] and sha(row['log'])==row['log_sha256'];assert re.findall(r'test result: ok\. (\d+) passed; 0 failed;',Path(row['log']).read_text())==[str(row['tests'])];pins[row['binary']]=row['binary_sha256'];pins[row['log']]=row['log_sha256']
 assert {x['suite']:x['tests'] for x in r['results']}=={'model_compare':30,'model_tiles':21,'search_portfolio':35,'portfolio_collection':36,'joint_witnesses':10,'geometry':9,'projective_domains':4,'joint_projective':4,'projective_pruning':3}
 for f in ['crates/rrrah-dedup/src/portfolio_collection.rs','crates/rrrah-dedup/tests/portfolio_collection.rs','crates/rrrah-dedup/src/model_banks.rs','crates/rrrah-dedup/src/joint_witnesses.rs','crates/rrrah-dedup/tests/joint_witnesses.rs','crates/rrrah-dedup/tests/model_compare.rs','crates/rrrah-dedup/src/search_portfolio.rs','crates/rrrah-dedup/tests/search_portfolio.rs']:assert sha(R/f)==m['file_hashes'][f]
 pins.update({str(x):sha(x) for x in [p,mp,Path(__file__)]});out=D/'dedup-model-bank-pixel-context-guards-verification-20261011.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_152_tests_model_bank_pixel_context_guards','pins':pins,'scope':'Explicit up-to-eight window/rank context recipes share25 extractions and geometric hypotheses; conservative max-rank preflight plus aggregate per-context work; sticky cancellation/source guards retained; chosen context recorded. Legacy one-context evidence parity and all152 generic pair/collection/geometry regressions. Actual CPU wrapper for new API, cohort precision and all-case qualification remain pending.'},indent=2)+'\n');print('Verified152')
if __name__=='__main__':main()
