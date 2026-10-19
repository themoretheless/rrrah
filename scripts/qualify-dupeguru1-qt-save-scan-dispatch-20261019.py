"""Exercise original Qt save cancellation and scan confirmation dispatch."""
from pathlib import Path
from types import SimpleNamespace
import hashlib
import json
import subprocess
import sys

R = Path(__file__).resolve().parents[1]
S = R / '.dedup-work/dupeguru1-20261019/source'
W = R / '.dedup-work/dupeguru1-qt-save-scan-dispatch-20261019'


def main():
    W.mkdir(exist_ok=False)
    names = ['qt/result_window.py', 'qt/directories_dialog.py']
    pins = {n: hashlib.sha256((S / n).read_bytes()).hexdigest() for n in names}
    for n in names:
        assert subprocess.check_output(['git', '-C', str(S), 'show', 'HEAD:' + n]) == (S / n).read_bytes()
    sys.path.insert(0, str(S))
    import qt.result_window as rw
    import qt.directories_dialog as dd

    calls, recent, confirmations = [], [], []
    model = SimpleNamespace(save_as=lambda p: calls.append(['save_results', p]),
                            save_directories_as=lambda p: calls.append(['save_directories', p]),
                            results=SimpleNamespace(is_modified=False),
                            start_scanning=lambda p: calls.append(['scan', p]))
    app = SimpleNamespace(model=model, recentResults=SimpleNamespace(insertItem=recent.append),
                          prefs=SimpleNamespace(profile_scan=False))
    receiver = SimpleNamespace(app=app)
    rw.QFileDialog.getSaveFileName = lambda *args, **kwargs: ('', '')
    rw.ResultWindow.saveResultsTriggered(receiver)
    dd.DirectoriesDialog.saveDirectoriesTriggered(receiver)
    assert calls == [] and recent == []
    model.results.is_modified = True
    app.confirm = lambda *args: confirmations.append(False) or False
    dd.DirectoriesDialog.scanButtonClicked(receiver)
    assert calls == [] and confirmations == [False]
    app.confirm = lambda *args: confirmations.append(True) or True
    dd.DirectoriesDialog.scanButtonClicked(receiver)
    assert calls == [['scan', False]] and confirmations == [False, True]
    model.results.is_modified = False
    app.confirm = lambda *args: (_ for _ in ()).throw(AssertionError('unexpected confirmation'))
    app.prefs.profile_scan = True
    dd.DirectoriesDialog.scanButtonClicked(receiver)
    assert calls == [['scan', False], ['scan', True]]
    assert all(hashlib.sha256((S / n).read_bytes()).hexdigest() == pins[n] for n in names)
    report = {'status': 'passed_original_Qt_save_cancel_and_scan_confirmation_dispatch',
              'source_pins': pins, 'cancel_save_calls': [], 'cancel_recent_calls': [],
              'scan_calls': calls, 'modified_results_confirmations': confirmations,
              'limits': ['Original Qt methods unchanged on receiver adapters, dialog response simulated.',
                         'Model write/scan and recent callbacks captured; no actual write, search, GUI or user settings.',
                         'Verifies dispatch only, not persistence, scan correctness or real confirmation interaction.',
                         'Supplemental authored controls, not additional upstream tests.']}
    (W / 'result.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'status': report['status']}))


if __name__ == '__main__':
    main()
