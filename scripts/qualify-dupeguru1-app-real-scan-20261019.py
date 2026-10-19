"""Run original app scan closures synchronously over task-owned exact copies."""
from pathlib import Path
import hashlib
import json
import os
import sys

R = Path(__file__).resolve().parents[1]
S = R / '.dedup-work/dupeguru1-20261019/source'
W = R / '.dedup-work/dupeguru1-app-real-scan-20261019'


def main():
    W.mkdir(exist_ok=False)
    data = W / 'data'
    data.mkdir()
    appdata = W / 'appdata'
    appdata.mkdir()
    names = ['core/app.py', 'core/scanner.py', 'core/fs.py', 'core/directories.py', 'core/tests/base.py']
    pins = {p: hashlib.sha256((S / p).read_bytes()).hexdigest() for p in names}
    sys.path.insert(0, str(S))
    from hscommon import desktop
    from hscommon.jobprogress.job import nulljob, Job, JobCancelled
    from core.tests.base import TestApp
    from core.scanner import ScanType

    desktop._special_folder_path = lambda *args, **kwargs: str(appdata)
    payload = bytes(range(256)) * 1024
    (data / 'original.bin').write_bytes(payload)
    (data / 'copy.bin').write_bytes(payload)
    os.link(data / 'original.bin', data / 'alias.bin')
    (data / 'different.bin').write_bytes(payload[:-1] + b'X')
    app = TestApp().app
    app.appdata = str(appdata)
    app.directories.add_path(data)
    app.options['scan_type'] = ScanType.CONTENTS
    # Only transport is replaced: original app closure, Scanner and filesystem remain real.
    app.progress_window.run = lambda jobid, title, target, args=(): target(nulljob, *args)
    groups = lambda: sorted(sorted(f.name for f in g) for g in app.results.groups)
    try:
        app.start_scanning()
        default = groups()
        assert default == [['alias.bin', 'copy.bin', 'original.bin']]
        app.options['ignore_hardlink_matches'] = True
        app.start_scanning()
        deduped = groups()
        assert len(deduped) == 1 and len(deduped[0]) == 2 and 'copy.bin' in deduped[0]
        app.progress_window.run = lambda jobid, title, target, args=(): target(Job(1, lambda *a, **k: False), *args)
        try:
            app.start_scanning()
        except JobCancelled:
            pass
        else:
            raise AssertionError('expected cancellation')
        assert groups() == []
        app.progress_window.run = lambda jobid, title, target, args=(): target(nulljob, *args)
        app.start_scanning()
        retry = groups()
        assert retry == deduped
    finally:
        app.close()
    assert all(hashlib.sha256((S / p).read_bytes()).hexdigest() == pin for p, pin in pins.items())
    result = {'status': 'passed_original_app_real_file_scan_controls', 'source_pins': pins,
              'default_groups': default, 'ignore_hardlink_groups': deduped, 'cancelled_groups': [], 'retry_groups': retry,
              'limits': ['Original scan closure executed synchronously; not threaded progress or real GUI qualification.',
                         'Ordinary copies, one real hardlink and same-length negative on one task-owned filesystem.',
                         'No copy/move/delete actions invoked; only task-owned hash cache and fixture writes.',
                         'These are authored supplemental controls, not additional upstream tests.']}
    (W / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result))


if __name__ == '__main__':
    main()
