#!/usr/bin/env python3
"""Fail closed unless exactly the required CUDA test completed on a recorded host."""
import argparse
import csv
import hashlib
import io
import json
from pathlib import Path
import re

TEST = 'tests::nvidia_full_output_boundaries_hdr_alpha_and_managed_limits'
MARKER = 'RRRAH_CUDA_QUALIFIED device_ordinal=0 cases=42 exact_rgb_alpha=true managed_final=0'

def verify(text, hardware, root, source_hashes):
    errors = []
    tests = re.findall(r'^test ([\w:]+) \.\.\. (ok|FAILED|ignored)(?: .*)?$', text, re.M)
    if tests != [(TEST, 'ok')]:
        errors.append('required exact test did not pass alone')
    summaries = re.findall(r'^test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored;', text, re.M)
    if summaries != [('ok', '1', '0', '0')]:
        errors.append('expected exactly one complete successful test summary')
    if text.splitlines().count(MARKER) != 1:
        errors.append('missing or duplicate CUDA completed-work marker')
    devices = list(csv.reader(io.StringIO(hardware)))
    if not devices or any(len(row)!=4 or not row[0].strip().isdigit() or not row[1].strip().startswith('GPU-') or not row[2].strip() or not re.fullmatch(r'\d+(?:\.\d+)+', row[3].strip()) for row in devices):
        errors.append('GPU UUID/name/driver metadata missing or invalid')
    required = {p.relative_to(root).as_posix() for p in (root/'crates/rrrah-cuda/src').rglob('*') if p.is_file()}
    if set(source_hashes) != required:
        errors.append('incomplete CUDA source manifest')
    else:
        for path, expected in source_hashes.items():
            if hashlib.sha256((root/path).read_bytes()).hexdigest() != expected:
                errors.append('source changed: '+path)
    return errors

def main():
    p=argparse.ArgumentParser();p.add_argument('directory', type=Path);a=p.parse_args();root=Path(__file__).resolve().parents[1]
    errors=verify((a.directory/'test.log').read_text(),(a.directory/'devices.csv').read_text(),root,json.loads((a.directory/'source-sha256.json').read_text()))
    (a.directory/'verification.json').write_text(json.dumps({'verified':not errors,'errors':errors,'scope':'exact CUDA test and recorded NVIDIA host; graphics/CUDA adapter matching is a separate gate'},indent=2)+'\n')
    if errors:raise SystemExit('; '.join(errors))
    print('Verified exact CUDA test, completed work, hardware metadata and source hashes')
if __name__=='__main__':main()
