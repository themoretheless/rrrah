"""Original rfigc CLI control with authored temporary files."""
import csv
import json
import sys
import tempfile
from pathlib import Path
cache = Path('.dedup-work/pyfilefixity044-20261023').resolve()
sys.path.insert(0, str(cache / 'dependencies'))
sys.path.insert(0, str(cache / 'pyFileFixity'))
import rfigc
with tempfile.TemporaryDirectory(prefix='fixity-header-review-') as folder:
    root = Path(folder)
    inputs = root / 'input'
    inputs.mkdir()
    target = inputs / 'one.bin'
    target.write_bytes(b'original')
    database = root / 'new.csv'
    appended = rfigc.main(['-i', str(inputs), '-d', str(database), '--update', '--append', '--silent'])
    with database.open(newline='') as handle:
        reader = csv.DictReader(handle, delimiter='|')
        fieldnames = reader.fieldnames
        rows = list(reader)
    target.write_bytes(b'changed content')
    checked = rfigc.main(['-i', str(inputs), '-d', str(database), '--silent'])
    assert appended == checked == 0
    assert fieldnames[0] == 'one.bin' and rows == []
    print(json.dumps({'append_return': appended, 'database_has_schema_header': fieldnames[0] == 'path', 'parsed_data_rows': len(rows), 'check_return_after_content_change': checked}, indent=2))
