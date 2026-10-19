"""Compare original viewer loading and Photo dimensions on existing EXIF fixtures."""
from pathlib import Path
from types import SimpleNamespace
import hashlib
import json
import os
import sys

R = Path(__file__).resolve().parents[1]
S = R / '.dedup-work/dupeguru1-20261019/source'
F = R / '.dedup-work/dupeguru1-qt-exif-orientations-v2-20261019'
W = R / '.dedup-work/dupeguru1-viewer-exif-20261019'


def main():
    W.mkdir(exist_ok=False)
    os.environ['QT_QPA_PLATFORM'] = 'offscreen'
    sys.path.insert(0, str(S))
    from PyQt5.QtGui import QGuiApplication
    from PyQt5.QtCore import qVersion
    from qt.pe.photo import File
    from qt.pe.image_viewer import BaseController
    app = QGuiApplication([])
    assert app.platformName() == 'offscreen'
    group = object()
    button = SimpleNamespace(setEnabled=lambda value: None)
    toolbar = SimpleNamespace(buttonImgSwap=button, buttonNormalSize=button)
    receiver = SimpleNamespace(same_dimensions=True, cached_group=group,
                               parent=SimpleNamespace(verticalToolBar=toolbar),
                               updateButtonsAsPerDimensions=lambda value: None,
                               updateBothImages=lambda value: None,
                               centerViews=lambda value: None)
    rows = []
    for orientation in range(1, 9):
        path = F / f'orientation-{orientation}.jpg'
        photo = File(path)
        BaseController.updateView(receiver, photo, photo, group)
        pixmap = receiver.selectedPixmap
        assert not pixmap.isNull() and receiver.referencePixmap.isNull()
        rows.append({'orientation': orientation, 'fixture_sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
                     'Photo_dimensions': list(photo.dimensions), 'viewer_pixmap_dimensions': [pixmap.width(), pixmap.height()]})
    mismatches = [r['orientation'] for r in rows if r['Photo_dimensions'] != r['viewer_pixmap_dimensions']]
    result = {'status': 'executed_original_viewer_EXIF_dimension_controls', 'Qt_runtime': qVersion(), 'platform': app.platformName(),
              'rows': rows, 'dimension_mismatch_orientations': mismatches,
              'source_pins': {n: hashlib.sha256((S / n).read_bytes()).hexdigest() for n in ['qt/pe/image_viewer.py', 'qt/pe/photo.py']},
              'limits': ['Actual original BaseController.updateView and QPixmap file loading, actual original Qt Photo dimensions.',
                         'Offscreen QGuiApplication only; receiver bypasses widgets/buttons/scaling/centering and group reset.',
                         'Dimension agreement does not prove pixel orientation, rendering correctness or equality.',
                         'Eight preexisting ordinary synthetic JPEGs only; no user images, invalid images or deferred memory probes.',
                         'No additional upstream tests or production changes.']}
    (W / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({'status': result['status'], 'dimension_mismatch_orientations': mismatches}))


if __name__ == '__main__':
    main()
