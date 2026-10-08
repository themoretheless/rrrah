"""Audit native Linux per-request cancellation on frozen source."""
import json, hashlib
from pathlib import Path
D = Path('docs/research')
manifest = D / 'dedup-collection-token-snapshot.json'
log = D / 'dedup-collection-token-linux.log'
out = D / 'dedup-collection-token-linux-state.json'
assert not out.exists()
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
m = json.loads(manifest.read_text())
assert all(sha(Path(m['snapshot']) / p) == h for p, h in m['file_hashes'].items())
t = log.read_text()
assert 'test either_request_generation_cancels_collection_and_releases_all_credits ... ok' in t
assert 'test result: ok. 1 passed; 0 failed; 0 ignored;' in t
assert 'error:' not in t and 'FAILED' not in t
out.write_text(json.dumps(dict(status='verified_native_linux_collection_request_tokens', tests=1, token_owners=2, checkpoints_per_owner=[0, 1, 1000, 10000], pins={str(p):sha(p) for p in [manifest, log, Path(__file__)]}, scope='Frozen native Linux request cancellation: global signal false; eight entry/early/extraction cases; Cancelled, credit release and entry zero peak. No late token cancellation, dependency mutation, Windows or full real collection claim.'), indent=2) + '\n')
print('verified native Linux request token cancellation')
