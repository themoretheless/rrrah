#!/usr/bin/env python3
"""CC0 repeated authored RGB pattern; pydicom is fixture tooling only."""
from pathlib import Path
import numpy as np
import pydicom
from pydicom.uid import RLELossless
root = Path(__file__).resolve().parents[1] / 'tests/fixtures/raster'
dataset = pydicom.dcmread(root / 'dicom-rle-rgb-explicit-8-planar0-plain.dcm')
dataset.decompress(generate_instance_uid=False)
original = dataset.pixel_array.copy()
frames = np.tile(original, (22, 1, 1, 1))[:64].copy()
dataset.NumberOfFrames = 64
dataset.PixelData = frames.tobytes()
dataset.compress(RLELossless, arr=frames, encoding_plugin='pydicom', generate_instance_uid=False)
assert np.array_equal(dataset.pixel_array, frames)
dataset.save_as(root / 'dicom-rle-prefetch-series64.dcm', enforce_file_format=True)
