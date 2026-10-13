#!/usr/bin/env python3
"""Verify the complete model comparison suite on the bidirectional snapshot."""
import hashlib,json,re,shutil
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-bidirectional-exposure-collection-verification-20261012.json';assert not out.exists()
 mp=D/'dedup-bidirectional-exposure-collection-snapshot-20261012.json';m=json.loads(mp.read_text());s=Path(m['snapshot'])
 assert sha(m['parent_manifest'])==m['parent_sha256']
 assert all(sha(s/k)==v for k,v in m['file_hashes'].items())
 assert all(sha(R/k)==v for k,v in m['file_hashes'].items() if k.startswith('crates/rrrah-dedup/src/') or k=='crates/rrrah-dedup/tests/model_compare.rs')
 log=D/'dedup-bidirectional-exposure-collection-tests-20261012.log';text=log.read_text();assert 'FAILED'not in text
 assert re.findall(r'test result: ok\. (\d+) passed; 0 failed;',text)==['46']
 for name in ['bidirectional_exposure_collection_preserves_direction_and_skips_supported_suffix','bidirectional_exposure_collection_prepays_reverse_before_input','bidirectional_exposure_collection_reverse_final_mutation_and_cancel_discard_batch']:assert f'test {name} ... ok'in text
 binary=Path(re.search(r'Running tests/model_compare.rs \(([^\n]+)\)',text)[1]);saved=s/'verified-model-compare-executable';assert not saved.exists();shutil.copy2(binary,saved)
 out.write_text(json.dumps({'status':'verified_46_model_comparison_tests_bidirectional_exposure_collection','tests':46,'pins':{str(p):sha(p)for p in [mp,log,saved,Path(__file__)]},'scope':'Fresh full model_compare suite including existing43 controls plus bidirectional forward/reverse primary/reverse calibrated support and double misses, all9 reverse batch caps before input, skipped suffix and report credit, reverse final mutation/owner cancellation. CPU wrapper runtime, real directional recall, full integration gate, heldout precision/platform/RSS/full500 remain unqualified.'},indent=2)+'\n')
 print('Verified46 model comparison tests with bidirectional collection')
if __name__=='__main__':main()
