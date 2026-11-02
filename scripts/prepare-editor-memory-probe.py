#!/usr/bin/env python3
"""Prepare (never launch) an isolated macOS viewer qualification bundle.
Launch the returned bundle through the UI, toggle H, then close its window.
/usr/bin/time records the child process lifetime maximum RSS on normal exit.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import plistlib
import shlex
import shutil

ROOT = Path(__file__).resolve().parents[1]
p = argparse.ArgumentParser()
p.add_argument('run', choices=['small', 'large', 'raw-cold', 'raw-warm', 'recovery'])
a = p.parse_args()
r = ROOT / 'docs/research/editor-009-011-2026-10-24'
bundle = ROOT / 'target/editor-qualification/Rrrah Memory Verify.app'
mac = bundle / 'Contents/MacOS'
mac.mkdir(parents=True, exist_ok=True)
plist = {'CFBundleIdentifier':'dev.rrrah.memory-verify','CFBundleName':'Rrrah Memory Verify','CFBundleExecutable':'start','CFBundlePackageType':'APPL','NSHighResolutionCapable':True}
(bundle / 'Contents/Info.plist').write_bytes(plistlib.dumps(plist))
if not (mac/'rrrah').exists():
    shutil.copyfile(ROOT/'target/debug/rrrah', mac/'rrrah.new')
    os.chmod(mac/'rrrah.new',0o755)
    os.replace(mac/'rrrah.new',mac/'rrrah')
# Reuse one frozen executable across every case; never replace a live Mach-O.
source = ROOT/'target/editor-qualification/oct24'/({'small':'hdr-small.pfm','large':'hdr-large.pfm','recovery':'edited.pfm'}.get(a.run,'bayer.dng'))
args = [str(mac/'rrrah'),str(source),'--prefetch-ahead','0','--prefetch-behind','0','--gpu-memory-mb','128','--managed-memory-mb','256','--cache-dir',str(ROOT/'target/editor-qualification/oct24-cache')]
if a.run in ('small','large','recovery'):
    args += ['--untagged-color','linear-srgb','--no-cache']
else:
    args += ['--gpu-raw-compute']
frame = r/f'{a.run}-frames.json'
log = r/f'{a.run}-viewer.log'
usage = r/f'{a.run}-usage.txt'
wrapper = '\n'.join(['#!/bin/sh','export RUST_LOG=info','export RRRAH_EDIT_FRAME_TRACE='+shlex.quote(str(frame)), 'exec /usr/bin/time -l -o '+shlex.quote(str(usage))+' '+shlex.join(args)+' > '+shlex.quote(str(log))+' 2>&1',''])
(mac/'start').write_text(wrapper)
os.chmod(mac/'start',0o755)
(r/f'{a.run}-manifest.json').write_text(json.dumps({'argv':args,'binary_sha256':hashlib.sha256((mac/'rrrah').read_bytes()).hexdigest(),'source_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),'gpu_budget_mib':128,'cpu_budget_mib':256,'cache':'shared cold/warm' if a.run.startswith('raw') else 'disabled','memory':'kernel lifetime maximum RSS; sampled native Metal resource allocation counter with device allocation challenge','bundle':str(bundle)},indent=2)+'\n')
print(bundle)
