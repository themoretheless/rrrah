"""Isolated preference roundtrip and non-writing export-cancel control."""
from pathlib import Path
from types import SimpleNamespace
import hashlib
import json
import subprocess
import sys

R = Path(__file__).resolve().parents[1]
S = R / '.dedup-work/dupeguru1-20261019/source'
W = R / '.dedup-work/dupeguru1-qt-preferences-export-20261019'


def main():
    W.mkdir(exist_ok=False)
    names = ['qt/preferences.py', 'qt/app.py', 'core/app.py']
    pins = {n: hashlib.sha256((S / n).read_bytes()).hexdigest() for n in names}
    for n in names:
        assert subprocess.check_output(['git', '-C', str(S), 'show', 'HEAD:' + n]) == (S / n).read_bytes()
    sys.path.insert(0, str(S))
    from PyQt5.QtCore import QSettings
    import qt.preferences as prefmod
    import qt.app as qtapp
    import core.app as coreapp
    from core.app import AppMode
    from core.scanner import ScanType

    # Only font lookup and settings destination are isolated; no user settings or QApplication.
    prefmod.QApplication = SimpleNamespace(font=lambda: SimpleNamespace(pointSize=lambda: 12))
    prefmod.create_qsettings = lambda: QSettings(str(W / 'preferences.ini'), QSettings.IniFormat)
    prefs = prefmod.Preferences()
    prefs.load()
    defaults = {k: getattr(prefs, k) for k in ['big_file_partial_hashes', 'big_file_size_threshold', 'ignore_small_files', 'small_file_threshold', 'ignore_hardlink_matches']}
    assert defaults == {'big_file_partial_hashes': False, 'big_file_size_threshold': 100, 'ignore_small_files': True, 'small_file_threshold': 10, 'ignore_hardlink_matches': False}
    receiver = SimpleNamespace(prefs=prefs, model=SimpleNamespace(options={}, app_mode=AppMode.STANDARD), details_dialog=None, _set_style=lambda style: None)
    qtapp.DupeGuru._update_options(receiver)
    initial = dict(receiver.model.options)
    assert initial['size_threshold'] == 10240 and initial['big_file_size_threshold'] == 0 and initial['scan_type'] == ScanType.CONTENTS
    prefs.big_file_partial_hashes = True
    prefs.big_file_size_threshold = 7
    prefs.ignore_small_files = False
    prefs.ignore_large_files = True
    prefs.large_file_threshold = 11
    prefs.set_scan_type(AppMode.STANDARD, ScanType.FILENAME)
    prefs.save()
    reloaded = prefmod.Preferences()
    reloaded.load()
    receiver.prefs = reloaded
    qtapp.DupeGuru._update_options(receiver)
    changed = dict(receiver.model.options)
    assert changed['size_threshold'] == 0 and changed['big_file_size_threshold'] == 7 * 1024**2 and changed['large_size_threshold'] == 11 * 1024**2 and changed['scan_type'] == ScanType.FILENAME

    qtapp.QFileDialog.getSaveFileName = lambda *a, **k: ('', '')
    empty_dialog_result = qtapp.DupeGuru.select_dest_file(SimpleNamespace(resultWindow=None), 'Export', 'csv')
    assert empty_dialog_result == '.csv'
    writes = []
    coreapp.export.export_to_csv = lambda path, cols, rows: writes.append({'path': path, 'columns': cols, 'rows': rows})
    model = SimpleNamespace(view=SimpleNamespace(select_dest_file=lambda *args: empty_dialog_result), _get_export_data=lambda: (['Name'], [['synthetic.bin']]))
    coreapp.DupeGuru.export_to_csv(model)
    assert writes == [{'path': '.csv', 'columns': ['Name'], 'rows': [['synthetic.bin']]}]
    assert all(hashlib.sha256((S / n).read_bytes()).hexdigest() == pins[n] for n in names)
    def serial(options):
        return {k: sorted(v) if isinstance(v, set) else v for k, v in options.items()}
    result = {'status': 'preferences_roundtrip_passed_export_cancel_write_request_reproduced', 'source_pins': pins,
              'default_preferences': defaults, 'default_options': serial(initial), 'roundtripped_options': serial(changed),
              'cancel_dialog_return': empty_dialog_result, 'captured_export_write_requests': writes,
              'limits': ['Original preference constructor/load/save and native task-owned QSettings INI; only font lookup and settings destination adapted.',
                         'Original Qt option mapping invoked on a receiver with no details widget/style effects; no full GUI launch.',
                         'Dialog cancellation simulated at getSaveFileName; original Qt and core export dispatch unchanged, final writer replaced by capture.',
                         'No CSV written, no user settings loaded, no external application or deletion/move invoked.',
                         'Supplemental controls, not additional upstream tests or production fixes.']}
    (W / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({'status': result['status'], 'cancel_return': empty_dialog_result, 'write_requests': len(writes)}))


if __name__ == '__main__':
    main()
