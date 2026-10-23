"""Run only the original output-check helper against an owned temporary folder."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

helper = Path(sys.argv[1]).resolve()
with tempfile.TemporaryDirectory(prefix='nextcloud-helper-') as tmp:
    root = Path(tmp)
    (root / 'outputs').mkdir()
    payload = 'abc(file_hash)\n     /owned/fixture'
    sorted_payload = subprocess.run(
        ['sort', '-h'], input=payload + '\n', text=True,
        capture_output=True, check=True,
    ).stdout
    expected = hashlib.sha256(sorted_payload.encode()).hexdigest()
    env = os.environ.copy()
    env.update(BATS_TEST_DIRNAME=str(root), BATS_TEST_FILENAME='owned.bats',
               BATS_TEST_NUMBER='1', SAVE_OUTPUTS='0')
    run = subprocess.run(
        ['bash', '-c', '. "$1"\nevaluateHashResult "$2" 1 "$3"',
         'helper-control', str(helper), expected, payload],
        env=env, text=True, capture_output=True,
    )
    assert run.returncode == 0, (run.returncode, run.stdout, run.stderr)
    output = root / 'outputs/owned.bats_1.out'
    assert output.read_text() == payload + '\n'
    print(json.dumps({
        'original_helper_sha256': hashlib.sha256(helper.read_bytes()).hexdigest(),
        'exit_code': run.returncode, 'SAVE_OUTPUTS': '0',
        'output_file_written': True, 'output_matches_authored_payload': True,
        'scope': 'Original complete helper sourced; only evaluateHashResult invoked with an owned temporary directory. No database, cleanup, HTTP, server, setup or BATS execution.',
    }, indent=2))
