"""Exercise original Qt directory-drop conversion with native QMimeData/QUrl."""
from pathlib import Path
from types import SimpleNamespace
import hashlib
import json
import subprocess
import sys

R = Path(__file__).resolve().parents[1]
S = R / '.dedup-work/dupeguru1-20261019/source'
W = R / '.dedup-work/dupeguru1-qt-uri-paths-20261019'


def main():
    W.mkdir(exist_ok=False)
    names = ['qt/directories_model.py', 'qt/tree_model.py']
    pins = {n: hashlib.sha256((S / n).read_bytes()).hexdigest() for n in names}
    for n in names:
        assert subprocess.check_output(['git', '-C', str(S), 'show', 'HEAD:' + n]) == (S / n).read_bytes()
    sys.path.insert(0, str(S))
    from PyQt5.QtCore import QMimeData, QUrl, Qt, qVersion, QT_VERSION_STR, PYQT_VERSION_STR
    from qt.directories_model import DirectoriesModel

    paths = [str(W / n) for n in ['plain', 'with space', 'русский_日本語', 'percent%value', 'hash#value', 'line\nbreak', 'carriage\rreturn']]
    mime = QMimeData()
    mime.setUrls([QUrl.fromLocalFile(p) for p in paths])
    added, signals, resets = [], [], []
    receiver = SimpleNamespace(MIME_TYPE_FORMAT=DirectoriesModel.MIME_TYPE_FORMAT,
                               model=SimpleNamespace(add_directory=added.append),
                               foldersAdded=SimpleNamespace(emit=signals.append),
                               reset=lambda: resets.append(True))
    accepted = DirectoriesModel.dropMimeData(receiver, mime, Qt.CopyAction, -1, 0, None)
    assert accepted and added == paths and signals == [paths] and resets == [True]
    missing = QMimeData()
    rejected = DirectoriesModel.dropMimeData(receiver, missing, Qt.CopyAction, -1, 0, None)
    assert rejected is False and added == paths and resets == [True]
    assert all(hashlib.sha256((S / n).read_bytes()).hexdigest() == pins[n] for n in names)
    report = {'status': 'passed_original_Qt_URI_conversion_controls', 'source_pins': pins,
              'Qt_runtime': qVersion(), 'Qt_compile': QT_VERSION_STR, 'PyQt': PYQT_VERSION_STR,
              'native_mime_bytes_hex': bytes(mime.data('text/uri-list')).hex(),
              'expected_paths': paths, 'observed_paths': added,
              'accepted': accepted, 'missing_format_rejected': rejected is False,
              'limits': ['Original dropMimeData method unchanged; native Qt MIME and URL encoding/decoding.',
                         'Receiver captures add_directory/signal/reset calls; no real directory model, filesystem traversal or GUI drag event.',
                         'No paths created except task-owned output folder; no clipboard or user application touched.',
                         'Supplemental authored controls, not additional upstream tests or platform-wide qualification.']}
    (W / 'result.json').write_text(json.dumps(report, indent=2, ensure_ascii=False) + '\n')
    print(json.dumps({'status': report['status'], 'path_cases': len(paths), 'Qt_runtime': qVersion()}))


if __name__ == '__main__':
    main()
