#!/usr/bin/env python3
import hashlib,json
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-loftr-strip-output-all-units-verification-20261012.json';assert not out.exists();mp=D/'dedup-loftr-strip-output-candidate-20261012.json';m=json.loads(mp.read_text());assert m['tests']==7 and all(sha(p)==v for p,v in m['pins'].items());s=Path(m['candidate']);assert all(sha(s/k)==v for k,v in m['source_file_hashes'].items());log=T/'rrrah-loftr-strip-output-all-unit-tests-20261012.log';text=log.read_text();assert '49 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out'in text and 'FAILED'not in text
 for name in ['output_copy_matches_original_strip_values_and_preserves_autograd','cancellation_at_every_inference_checkpoint_discards_output_and_allows_retry','regression_no_eager_debug_preserves_outputs_and_public_debug']:assert name+' ... ok'in text
 out.write_text(json.dumps({'status':'verified_all_49_native_loftr_unit_tests_strip_output_candidate','tests':49,'pins':{str(p):sha(p)for p in [mp,log,Path(__file__)]},'scope':'Same frozen candidate/executable as7backbone controls; all49nativeCPUunit tests pass, including whole-model shape/debug/cancellation/retry, transformers, coarse/fine selection and gradient/value/seam tests. Isolated candidate, not root adoption; actual original-weight full CPU comparison and totalRSS qualification remain pending.'},indent=2)+'\n');print('Verified all49native LoFTR unit tests')
if __name__=='__main__':main()
