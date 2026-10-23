"""Run original pinned rfigc against temporary authored byte fixtures only."""
import csv
import json
import sys
import tempfile
from pathlib import Path

cache = Path('.dedup-work/pyfilefixity044-20261023').resolve()
sys.path.insert(0, str(cache / 'dependencies'))
sys.path.insert(0, str(cache / 'pyFileFixity'))
import rfigc

with tempfile.TemporaryDirectory(prefix='fixity-review-') as temporary:
    root = Path(temporary)
    original, scraped, restored = (root / name for name in ['original', 'scraped', 'restored'])
    for folder in [original, scraped, restored]:
        folder.mkdir()
    for name in ['first.bin', 'second.bin']:
        (original / name).write_bytes(b'identical authored file data')
    (scraped / 'unknown.bin').write_bytes(b'identical authored file data')
    database = root / 'database.csv'
    generated = rfigc.main(['-i', str(original), '-d', str(database), '-g', '-f', '--silent'])
    recovered = rfigc.main(['-i', str(scraped), '-d', str(database), '--filescraping_recovery', '-o', str(restored), '--silent'])
    recovered_names = sorted(p.name for p in restored.iterdir())
    assert generated == recovered == 0
    assert recovered_names == ['second.bin'], recovered_names
    # Single-file remove-update should reveal whether unrelated live rows survive.
    updated = rfigc.main(['-i', str(original / 'first.bin'), '-d', str(database), '--update', '--remove', '--silent'])
    with database.open(newline='') as handle:
        remaining_rows = list(csv.DictReader(handle, delimiter='|'))
    remaining_names = [row['path'] for row in remaining_rows]
    assert updated == 0 and remaining_names == ['first.bin'], remaining_names
    assert (original / 'second.bin').exists()
    print(json.dumps({'generated_exit': generated, 'recovery_exit': recovered, 'expected_original_names': ['first.bin', 'second.bin'], 'recovered_names': recovered_names, 'single_file_update_exit': updated, 'remaining_database_names': remaining_names, 'unrelated_original_still_exists': True}, indent=2))
