"""Independent oracle for the authored i-newline triangular fixture snapshot."""
from pathlib import Path
import hashlib
import json
import re
import subprocess
import os
import sys

raw = Path(sys.argv[1]).read_bytes()
lines = raw.decode().splitlines()
scanned = [s.removeprefix('Scanning ') for s in lines if s.startswith('Scanning ')]
expected = {f'/admin/files/tests/{i}_{j}.txt' for i in range(26) for j in range(i + 1)}
assert len(scanned) == len(expected) == 351
assert set(scanned) == expected
actual = {}
current = None
for line in lines:
    match = re.fullmatch(r'([0-9a-f]{64})\(file_hash\)', line)
    if match:
        current = match[1]
        assert current not in actual
        actual[current] = []
    elif line.startswith('     /'):
        assert current is not None
        actual[current].append(line.strip())
expected_groups = {
    hashlib.sha256(f'{i}\n'.encode()).hexdigest():
    {f'/admin/files/tests/{i}_{j}.txt' for j in range(i + 1)}
    for i in range(1, 26)
}
assert actual.keys() == expected_groups.keys()
for digest, paths in actual.items():
    assert len(paths) == len(set(paths))
    assert set(paths) == expected_groups[digest]
heading = b'Duplicates for user "admin" are: '
offset = raw.index(heading)
assert raw[offset:] == Path(sys.argv[2]).read_bytes()
env = os.environ.copy()
env['LC_ALL'] = 'C'
ordered = subprocess.run(['sort', '-h'], input=raw, env=env,
                         capture_output=True, check=True).stdout
assert hashlib.sha256(ordered).hexdigest() == '19fb85351e9ef4cbca4b165043d303fd723413fb3df6e0929d34a0e83c025142'
print(json.dumps({'snapshot_sha256': hashlib.sha256(raw).hexdigest(),
                  'scanned_paths': len(scanned), 'groups': len(actual),
                  'group_members': sum(map(len, actual.values())),
                  'content_and_membership_mismatches': 0,
                  'list_snapshot_byte_equal': True,
                  'sorted_text_hash_matches_original_scenario': True,
                  'scope': 'Pinned saved outputs only; no original BATS or Nextcloud execution.'}, indent=2))
