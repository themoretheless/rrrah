#!/usr/bin/env python3
"""Qualify explicit atomic exact cancellation including publication handoff."""
import hashlib
import json
import re
import shutil
from pathlib import Path
R = Path(__file__).resolve().parents[1]
T = Path('/Users/themoretheless/.codex/tmp')
D = R / 'docs/research'
S = T / 'rrrah-exact-cancel-flag-20261013'
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
prior_path = D / 'dedup-exact-healthy-publication-audit-20261013.json'
prior = json.loads(prior_path.read_text())
affected = ['crates/rrrah-dedup/src/exact.rs', 'crates/rrrah-dedup/tests/exact_files.rs']
pins = dict(prior['source_sha256'])
assert all(sha(S/p) == h for p, h in pins.items() if p not in affected)
for p in affected:
    pins[p] = sha(S/p)
    assert sha(R/p) == pins[p]
log = T / 'rrrah-exact-cancel-flag-20261013.log'
text = log.read_text()
assert re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', text) == [('40','0','2'),('29','0','0')]
assert 'exact::cancellation_flag_tests::final_publication_observes_flag_without_reentering_caller ... ok' in text
assert 'atomic_cancel_exact_apis_preserve_groups_refuse_pre_cancel_and_allow_retry ... ok' in text
ignored = re.findall(r'^test (.+) \.\.\. ignored, (.+)$',text,re.M)
assert len(ignored) == 2 and all(reason == 'manual alternating exact matcher timing control' for _, reason in ignored)
binaries = re.findall(r'Running (?:unittests src/lib.rs|tests/exact_files.rs) \(([^)]+)\)', text)
assert len(binaries) == 2
dest = T / 'rrrah-exact-cancel-flag-qualified-20261013'
assert not dest.exists()
dest.mkdir()
saved = {}
for binary, name in zip(binaries, ['library_tests','exact_files']):
    output = dest/name
    shutil.copy2(binary, output)
    assert sha(binary) == sha(output)
    saved[str(output)] = sha(output)
result = dict(status='verified_atomic_exact_cancellation',passed=69,failed=0,ignored=2,
    counts=dict(default_library=40,exact_files=29),ignored_tests=ignored,
    source=str(S),source_files=len(pins),source_sha256=pins,
    affected_root_sha256={p:sha(R/p) for p in affected},executables=saved,
    log=str(log),log_sha256=sha(log),prior_audit=str(prior_path),prior_audit_sha256=sha(prior_path),
    controls='Both public flag APIs: healthy, pre-cancelled nonempty/empty input, reset retry. Private final-checkpoint handoff: caller returns false but sets flag; publication cancels, clears groups, reads less than baseline, does not reenter caller, retry restores groups.',
    scope='Flag must remain set until return; block-boundary cooperative cancellation, not preemption of blocked OS I/O. Final-phase handoff injected deterministically; no measured concurrent cancellation latency or hard deadline. Old callback API retains uninterruptible owner-only final pass. No full decode/native/platform gate.')
path=D/'dedup-exact-cancel-flag-audit-20261013.json'
assert not path.exists()
path.write_text(json.dumps(result,indent=2)+'\n')
print(result['status'],result['passed'],result['ignored'])
