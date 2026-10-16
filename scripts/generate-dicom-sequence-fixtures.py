"""CC0 authored nested DICOM encodings; pydicom-only pixel oracle.
Run after generate-dicom-fixtures.py, with pydicom3.0.2 / numpy2.2.6.
Tests structural encoding and root-field isolation, not clinical IOD semantics.
"""
from pathlib import Path
import numpy as np
import pydicom
from pydicom.dataset import Dataset
from pydicom.sequence import Sequence

root = Path(__file__).resolve().parents[1] / 'tests/fixtures/raster'
manifest = root / 'dicom-manifest.tsv'
lines = [line for line in manifest.read_text().splitlines() if '-sequence-' not in line]
for encoding, source in [('explicit', 'dicom-unsigned16.dcm'),
                         ('implicit', 'dicom-implicit-unsigned16.dcm'),
                         ('big-endian', 'dicom-big-endian-unsigned16.dcm')]:
 for seq_undefined in [False, True]:
  for item_undefined in [False, True]:
   name = f'dicom-{encoding}-sequence-{"u" if seq_undefined else "d"}{"u" if item_undefined else "d"}.dcm'
   ds = pydicom.dcmread(root / source)
   ds.SOPInstanceUID = ds.file_meta.MediaStorageSOPInstanceUID = f'1.2.826.0.1.3680043.10.543.91{len(lines)}'
   item = Dataset()
   item.PatientID = 'RRRAH-NESTED'
   # Nested scalar/pixel tags must never replace the root metadata or pixels.
   item.Rows = 1; item.Columns = 1; item.PixelData = b'\xff\xff'
   item['PixelData'].VR = 'OW'
   nested = Dataset(); nested.ReferencedSOPClassUID = ds.SOPClassUID
   nested.ReferencedSOPInstanceUID = ds.SOPInstanceUID
   nested.is_undefined_length_sequence_item = not item_undefined
   item.SourceImageSequence = Sequence([nested])
   item['SourceImageSequence'].is_undefined_length = not seq_undefined
   item.is_undefined_length_sequence_item = item_undefined
   ds.OtherPatientIDsSequence = Sequence([item, Dataset()])
   ds['OtherPatientIDsSequence'].is_undefined_length = seq_undefined
   pydicom.dcmwrite(root / name, ds, enforce_file_format=True)
   restored = pydicom.dcmread(root / name)
   gray = restored.pixel_array.astype(np.float64).reshape(-1)
   assert gray.tolist() == [0, 1, 32767, 32768, 65534, 65535]
   for kind, values in [('raw', gray), ('window', np.clip(gray / 65535, 0, 1)),
                        ('alternate-window', np.clip((gray - 16383.75) / 32767.5, 0, 1))]:
    rgba = np.stack([values, values, values, np.ones_like(values)], axis=1).astype('<f4')
    (root / f'{name}.0.{kind}-f32').write_bytes(rgba.tobytes())
   lines.append(f'{name}\t3\t2\t1\t0\t65535')
manifest.write_text('\n'.join(lines) + '\n')
print('wrote 12 sequence containers; all root pixels checked independently')
