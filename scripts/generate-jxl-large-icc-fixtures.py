#!/usr/bin/env python3
"""Generate a deterministic ICC entropy-cancellation control using libjxl."""
from pathlib import Path
import hashlib, subprocess, sys, tempfile
root=Path(__file__).resolve().parents[1]
profile=bytearray((root/'tests/fixtures/raster/linear-identity-rgb.icc').read_bytes())
# Unreferenced profile tail preserves all existing tags and interpretation.
profile.extend(b''.join(hashlib.sha256(b'rrrah-icc-tail'+i.to_bytes(4,'little')).digest() for i in range(256)))
profile[:4]=len(profile).to_bytes(4,'big')
with tempfile.TemporaryDirectory() as directory:
    path=Path(directory)/'large.icc';path.write_bytes(profile)
    subprocess.run([sys.executable,str(root/'scripts/generate-jxl-embedded-icc-fixtures.py'),
                    '--profile',str(path),'--name','large-icc'],check=True)
