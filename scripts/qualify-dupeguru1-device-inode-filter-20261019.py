"""Exercise the original alias filter with explicit synthetic stat identities."""
from pathlib import Path
from types import SimpleNamespace
import hashlib
import json
import subprocess
import sys

R = Path(__file__).resolve().parents[1]
S = R / '.dedup-work/dupeguru1-20261019/source'
W = R / '.dedup-work/dupeguru1-device-inode-filter-20261019'


def main():
    W.mkdir(exist_ok=False)
    source = S / 'core/app.py'
    before = source.read_bytes()
    assert subprocess.check_output(['git', '-C', str(S), 'show', 'HEAD:core/app.py']) == before
    sys.path.insert(0, str(S))
    from core.app import DupeGuru

    def file(name, device, inode):
        stat = SimpleNamespace(st_dev=device, st_ino=inode)
        return SimpleNamespace(name=name, path=SimpleNamespace(stat=lambda: stat))

    original = file('original', 100, 42)
    alias = file('same-device-alias', 100, 42)
    other_disk = file('other-device-distinct-file', 200, 42)
    other_inode = file('same-device-distinct-file', 100, 43)
    cases = []
    for inputs in ([original, alias, other_disk, other_inode], [other_disk, original, alias, other_inode]):
        observed = DupeGuru._remove_hardlink_dupes(inputs)
        expected = []
        seen = set()
        for item in inputs:
            stat = item.path.stat()
            identity = (stat.st_dev, stat.st_ino)
            if identity not in seen:
                seen.add(identity)
                expected.append(item.name)
        names = [item.name for item in observed]
        assert len(names) == 2 and len(expected) == 3 and names != expected
        cases.append({'input': [item.name for item in inputs], 'observed': names, 'device_inode_reference': expected})
    assert source.read_bytes() == before
    result = {
        'status': 'reproduced_cross_device_inode_filter_loss_on_synthetic_stat_records',
        'source_sha256': hashlib.sha256(before).hexdigest(),
        'cases': cases,
        'limits': [
            'Original static filter invoked unchanged; synthetic stat records, not a real multi-volume filesystem test.',
            'No application constructor, cache connection, file action, external command or GUI invoked.',
            'Application option ignore_hardlink_matches defaults to false; this concerns its enabled path.',
            'No production change or upstream patch; our Unix exact identity already includes device and inode.',
        ],
    }
    (W / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result))


if __name__ == '__main__':
    main()
