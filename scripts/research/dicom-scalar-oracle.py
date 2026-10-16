"""Independent scalar pydicom oracle; source read in place, never copied.
Usage: script SOURCE OUTPUT_DIR WINDOW_MIN WINDOW_MAX
Requires test-only pydicom3.0.2 and numpy2.2.6.
"""
from pathlib import Path
import hashlib
import json
import sys
import numpy as np
import pydicom
from pydicom.pixels import apply_modality_lut
source, target = Path(sys.argv[1]), Path(sys.argv[2])
lo, hi = map(float, sys.argv[3:5])
assert np.isfinite([lo, hi]).all() and lo < hi
ds = pydicom.dcmread(source)
assert ds.SamplesPerPixel == 1
raw = ds.pixel_array
physical = apply_modality_lut(raw, ds).astype(np.float64)
assert np.isfinite(physical).all()
window = np.clip((physical - lo) / (hi - lo), 0, 1)
if ds.PhotometricInterpretation == 'MONOCHROME1': window = 1 - window
else: assert ds.PhotometricInterpretation == 'MONOCHROME2'
target.mkdir(parents=True, exist_ok=True)
hashes = {}
for name, gray in [('raw', physical), ('window', window)]:
 rgba = np.stack([gray, gray, gray, np.ones_like(gray)], axis=-1).astype('<f4')
 data = rgba.tobytes(); (target / f'{name}.f32').write_bytes(data)
 hashes[name] = hashlib.sha256(data).hexdigest()
padding_count = None
if 'PixelPaddingValue' in ds:
 first = int(ds.PixelPaddingValue); last = int(ds.get('PixelPaddingRangeLimit', first))
 padding_count = int(np.count_nonzero((raw >= min(first, last)) & (raw <= max(first, last))))
report = {'source': str(source), 'source_sha256': hashlib.sha256(source.read_bytes()).hexdigest(),
 'source_bytes': source.stat().st_size, 'dimensions': [int(ds.Columns), int(ds.Rows)],
 'frames': int(ds.get('NumberOfFrames', 1)), 'samples': int(raw.size),
 'physical_minimum': float(physical.min()), 'physical_maximum': float(physical.max()),
 'window': [lo, hi], 'declared_padding_sample_count': padding_count,
 'root_sequence_tags': [str(el.tag) for el in ds if el.VR == 'SQ'],
 'oracles': hashes, 'pydicom': pydicom.__version__, 'numpy': np.__version__}
(target / 'result.json').write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps(report))
