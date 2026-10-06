#!/usr/bin/env python3
"""Author deterministic ZIP fixtures around pinned CC0 RAW; not Capture One EIPs."""
import argparse
import hashlib
import json
from pathlib import Path
import zipfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('source', type=Path)
parser.add_argument('output', type=Path)
parser.add_argument('--case-id', default='4366')
parser.add_argument('--include-local', action='store_true', help='Allow explicitly selected user-provided fixture; generated package stays local')
args = parser.parse_args()
root = Path(__file__).resolve().parents[1]
manifest = json.loads((root / 'tests/fixtures/raw-color-corpus.json').read_text())
cases = manifest['cases'] + (manifest.get('local_cases', []) if args.include_local else [])
case = next((c for c in cases if str(c['id']) == args.case_id), None)
if case is None or (case['license'] != 'CC0-1.0' and not case.get('local_source')):
    parser.error('case must identify a pinned public CC0 or explicitly allowed local RAW source')
extension = Path(case['filename']).suffix[1:].lower()
member = f'0.{extension}'
raw = args.source.read_bytes()
if hashlib.sha256(raw).hexdigest() != case['sha256']:
    parser.error(f'source must match pinned CC0 object {args.case_id}')
args.output.mkdir(parents=True, exist_ok=True)
records = []
for label, method in [('stored', zipfile.ZIP_STORED), ('deflated', zipfile.ZIP_DEFLATED)]:
    path = args.output / f'{extension}-{label}.eip'
    with zipfile.ZipFile(path, 'w', compression=method, compresslevel=6) as archive:
        for name, data in [(member, raw), ('CaptureOne/Settings/test.cos', b'authored placeholder; no adjustment interpretation')]:
            info = zipfile.ZipInfo(name, date_time=(2000, 1, 1, 0, 0, 0))
            info.compress_type = method
            info.create_system = 3
            info.external_attr = 0o100644 << 16
            archive.writestr(info, data, compress_type=method, compresslevel=6)
    with zipfile.ZipFile(path) as archive:
        assert archive.read(member) == raw
        assert archive.testzip() is None
    records.append({'file': path.name, 'bytes': path.stat().st_size, 'sha256': hashlib.sha256(path.read_bytes()).hexdigest()})
report = {'scope': 'Authored ZIP payload/CRC oracle; not an authentic Capture One EIP or adjustment oracle', 'source_id': case['id'], 'source_sha256': case['sha256'], 'source_license': case['license'], 'packages': records}
(args.output / 'manifest.json').write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps(report))
