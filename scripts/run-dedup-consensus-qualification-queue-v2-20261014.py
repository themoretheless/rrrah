#!/usr/bin/env python3
"""Continue only after the existing producer reports authoritative termination."""
from pathlib import Path
import json
import os
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'docs/research/dedup-consensus-qualification-queue-v2-20261014.json'
assert not OUT.exists()
state = dict(status='waiting_for_existing_first_variant', producer_pid=os.getpid(), completed=[], scope='Preserve each version binaries before shared Cargo target reuse; stop on any failed producer/audit. No native-feature inference qualification.')

def save():
    temporary = OUT.with_suffix('.tmp')
    temporary.write_text(json.dumps(state, indent=2) + '\n')
    temporary.replace(OUT)

save()
first = ROOT / 'docs/research/dedup-collection_first-split-gate-audit-20261014.json'
try:
    while not first.is_file():
        time.sleep(10)
    report=json.loads(first.read_text())
    assert report['status']=='verified_frozen_complete_inventory_in_split_permissioned_runs' and report['failed']==0 and report['targets']==3
    stages = [
        'run-dedup-consensus-collection-v3-focused-20261014.py',
        'audit-dedup-consensus-collection-v3-focused-20261014.py',
        'run-dedup-consensus-duration-v4-focused-20261014.py',
        'audit-dedup-consensus-duration-v4-focused-20261014.py',
        'run-dedup-consensus-native-collection-v5-focused-20261014.py',
        'audit-dedup-consensus-native-collection-v5-focused-20261014.py',
    ]
    for stage in stages:
        state.update(status='executing', stage=stage)
        save()
        subprocess.run([sys.executable, str(ROOT / 'scripts' / stage)], cwd=ROOT, check=True)
        if stage.startswith('run-'):
            name = stage.removeprefix('run-').removesuffix('.py') + '.json'
            result = json.loads((ROOT / 'docs/research' / name).read_text())
            assert result['status'] == 'passed_pending_audit' and result['returncode'] == 0, result
        state['completed'].append(stage)
        save()
    state.update(status='completed_decode_stages_native_feature_still_pending')
    save()
except BaseException as error:
    state.update(status='stopped_on_failure', error=repr(error))
    save()
    raise
