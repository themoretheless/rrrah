"""Verify the complete saved three-directory triangular fixture partition."""
from pathlib import Path
import hashlib
import json
import os
import re
import subprocess
import sys

raw = Path(sys.argv[1]).read_bytes()
actual = {}
current = None
for line in raw.decode().splitlines():
    match = re.fullmatch(r'([0-9a-f]{64})\(file_hash\)', line)
    if match:
        current = match[1]
        assert current not in actual
        actual[current] = []
    elif line.startswith('     /'):
        assert current is not None
        actual[current].append(line.strip())
    else:
        assert line == 'Duplicates are: '
roots = ['/admin/files/tests', '/tuser/files/tests', '/tuser/files/tests2']
expected = {
    hashlib.sha256(f'{i}\n'.encode()).hexdigest():
    {f'{root}/{i}_{j}.txt' for root in roots for j in range(i + 1)}
    for i in range(26)
}
assert actual.keys() == expected.keys()
for digest, paths in actual.items():
    assert len(paths) == len(set(paths))
    assert set(paths) == expected[digest]
all_paths = [p for paths in actual.values() for p in paths]
assert len(all_paths) == len(set(all_paths)) == 1053
zero = hashlib.sha256(b'0\n').hexdigest()
assert len(actual[zero]) == 3
env = os.environ.copy()
env['LC_ALL'] = 'C'
ordered = subprocess.run(['sort', '-h'], input=raw, capture_output=True,
                         env=env, check=True).stdout
assert hashlib.sha256(ordered).hexdigest() == '37df47434c76bd4387c975aba58906ea1c0e86be6e8e7d3f7ad329017f5a9187'
print(json.dumps({'snapshot_sha256': hashlib.sha256(raw).hexdigest(),
                  'groups': len(actual), 'paths': len(all_paths),
                  'zero_family_members': len(actual[zero]),
                  'content_and_membership_mismatches': 0,
                  'sorted_text_hash_matches_original_scenario': True,
                  'scope': 'Pinned saved listing only; no actual server, BATS, permissions or file reads.'}, indent=2))
