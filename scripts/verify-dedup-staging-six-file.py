#!/usr/bin/env python3
"""Verify finite staging-index collection parity and independently report RSS."""
import argparse
import hashlib
import json
import pathlib
import re

p = argparse.ArgumentParser()
p.add_argument('report')
p.add_argument('state')
p.add_argument('time_log')
p.add_argument('reference')
p.add_argument('output')
a = p.parse_args()
paths = [a.report, a.state, a.time_log, a.reference]
raw = {f: pathlib.Path(f).read_bytes() for f in paths}
digest = lambda b: hashlib.sha256(b).hexdigest()
state = json.loads(raw[a.state])
assert state['status'] == 'terminal' and type(state['exit_code']) is int and state['exit_code'] == 0
assert state['input_hashes'] == state['input_hashes_after']
for f, expected in state['input_hashes'].items():
    assert digest(pathlib.Path(f).read_bytes()) == expected, f
new = json.loads(raw[a.report])
old = json.loads(raw[a.reference])['native_evidence']
assert new['status'] == 'ok'
for key in ('files', 'indexed_features', 'descriptor_hits', 'proposed_pairs'):
    assert type(new[key]) is int and new[key] >= 0, key
assert type(new['edges']) is list and len(new['edges']) == 15
seen = set()
for edge in new['edges']:
    assert type(edge) is list and len(edge) == 4
    left, right, candidate, lanes = edge
    assert type(left) is int and type(right) is int and 1 <= left < right <= 6
    assert (left, right) not in seen
    seen.add((left, right))
    assert type(candidate) is bool and type(lanes) is list and len(lanes) == 5
    assert all(type(flag) is bool for flag in lanes)
    assert candidate == any(lanes)
assert {k: v for k, v in new.items() if k != 'managed_peak'} == {k: v for k, v in old.items() if k != 'managed_peak'}
assert type(new['managed_used']) is int and new['managed_used'] == 0
assert type(new['managed_peak']) is int and 0 <= new['managed_peak'] <= 64 * 1024 * 1024
assert new['files'] == 6 and new['proposed_pairs'] == 15
positive = [e[:2] for e in new['edges'] if e[2]]
assert positive == [[1, 2], [3, 4], [5, 6]]
rss = re.findall(rb'^\s*(\d+)\s+maximum resident set size\s*$', raw[a.time_log], re.M)
assert len(rss) == 1 and int(rss[0]) > 0
for f, b in raw.items():
    assert pathlib.Path(f).read_bytes() == b, f
result = {'status': 'verified_finite_six_file_collection', 'source_report_hashes': {f: digest(b) for f, b in raw.items()}, 'input_hashes': state['input_hashes'], 'positive_edges': positive, 'all_native_fields_equal_except_managed_peak': True, 'managed_peak': new['managed_peak'], 'managed_used': new['managed_used'], 'maximum_resident_set_bytes': int(rss[0]), 'scope': 'Six files and 15 proposals. No full-corpus, all-allocation, universal RSS ceiling or throughput claim.'}
pathlib.Path(a.output).write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result, indent=2))
