"""Verify the native feature lifetime regression and frozen build provenance."""
import hashlib
import json
from pathlib import Path

D = Path('docs/research')
manifest = D / 'dedup-feature-release-snapshot.json'
log = D / 'dedup-feature-release-unit-tests.log'
out = D / 'dedup-feature-release-unit-state.json'
assert not out.exists()
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
m = json.loads(manifest.read_text())
for relative, digest in m['file_hashes'].items():
    assert sha(Path(m['snapshot']) / relative) == digest, relative
text = log.read_text()
name = 'local_collection::feature_release_tests::fresh_confirmation_reuses_feature_credit_and_prechecks_keep_their_banks'
assert f'test {name} ... ok' in text
assert 'test result: ok. 1 passed; 0 failed; 0 ignored;' in text
assert 'error:' not in text and 'FAILED' not in text
source = Path(m['snapshot']) / 'crates/rrrah-dedup/src/local_collection.rs'
assert sha(source) == sha('crates/rrrah-dedup/src/local_collection.rs')
out.write_text(json.dumps({
    'status': 'verified_native_feature_release_unit',
    'tests': 1,
    'verified_source_files': len(m['file_hashes']),
    'pins': {str(p): sha(p) for p in [manifest, log, Path(__file__)]},
    'scope': 'Native shared collection lifecycle test: fresh confirmation reuses both managed bank credits at the same ceiling; feature prechecks retain banks and skip fresh comparison; pair evidence/counts and final zero credit checked. Does not qualify real decoding, full collection parity or cancellation/mutation.'
}, indent=2) + '\n')
print('verified native feature release unit')
