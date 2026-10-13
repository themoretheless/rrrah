"""Fetch upstream RoMa diagnostic weights once, without changing the library runtime."""
import concurrent.futures
import hashlib
import json
import os
from pathlib import Path
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
CACHE = Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays/learned-matching-models/roma-oracle-20261010')
OUT = ROOT / 'docs/research/dedup-roma-oracle-weights-20261010.json'
URLS = {
    'roma_outdoor.pth': 'https://github.com/Parskatt/storage/releases/download/roma/roma_outdoor.pth',
    'dinov2_vitl14_pretrain.pth': 'https://dl.fbaipublicfiles.com/dinov2/dinov2_vitl14/dinov2_vitl14_pretrain.pth',
}


def fetch(item):
    name, url = item
    path = CACHE / name
    assert not path.exists(), path
    temporary = path.with_suffix('.part')
    with urllib.request.urlopen(url, timeout=60) as response, temporary.open('xb') as output:
        size = int(response.headers['Content-Length'])
        assert 0 < size <= 2 * 1024**3
        copied = 0
        digest = hashlib.sha256()
        while chunk := response.read(1024**2):
            copied += len(chunk)
            assert copied <= size
            digest.update(chunk)
            output.write(chunk)
        assert copied == size
    temporary.rename(path)
    print(name, copied, digest.hexdigest(), flush=True)
    return {'path': str(path), 'url': url, 'bytes': copied, 'sha256': digest.hexdigest()}


assert not OUT.exists()
CACHE.mkdir(parents=True, exist_ok=True)
report = {'status': 'running', 'producer_pid': os.getpid(), 'scope': __doc__}
OUT.write_text(json.dumps(report, indent=2) + '\n')
try:
    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
        report['weights'] = list(pool.map(fetch, URLS.items()))
    report['status'] = 'complete'
except Exception as error:
    report.update(status='failed', error=str(error))
    raise
finally:
    OUT.write_text(json.dumps(report, indent=2) + '\n')
