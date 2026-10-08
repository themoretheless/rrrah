"""Audit current native request token cancellation at measured late checkpoints."""
import hashlib
import json
import re
from pathlib import Path
D = Path('docs/research')
manifest = D / 'dedup-collection-late-token-snapshot.json'
log = D / 'dedup-collection-late-token-tests.log'
out = D / 'dedup-collection-late-token-state.json'
assert not out.exists()
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
m = json.loads(manifest.read_text())
assert all(sha(Path(m['snapshot']) / p) == h for p, h in m['file_hashes'].items())
t = log.read_text()
names = set(re.findall(r'^test (\w+) \.\.\. ok$', t, re.M))
assert names == {'either_request_generation_cancels_collection_and_releases_all_credits', 'either_request_generation_cancels_mid_and_final_collection_checkpoints'}
assert 'test result: ok. 2 passed; 0 failed; 0 ignored;' in t
assert 'error:' not in t and 'FAILED' not in t
baselines = re.findall(r'request owner=(\d+), baseline checkpoints=(\d+)', t)
assert [int(owner) for owner, _ in baselines] == [0, 1]
assert all(int(total) > 10000 for _, total in baselines)
out.write_text(json.dumps(dict(status='verified_native_macos_collection_late_request_tokens', tests=2, baseline_callbacks=dict(baselines), pins={str(p):sha(p) for p in [manifest, log, Path(__file__)]}, scope='Current frozen library: eight early request-token cases, two owner-specific supported mirrored baselines, both owners cancelled at measured midpoint and final checkpoint with global callback false; typed Cancelled and zero credit required. No source/dependency mutation, Linux, Windows or full real collection claim.'), indent=2) + '\n')
print('verified native late request tokens', baselines)
