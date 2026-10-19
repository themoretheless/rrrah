#!/usr/bin/env python3
"""Create the independent OpenEXR C++ associated-emission precision oracle."""
from pathlib import Path
import hashlib,json,os,shlex,subprocess,tempfile
root=Path(__file__).resolve().parents[1];source=root/'scripts/exr-associated-emission-oracle.cpp'
fixture=root/'tests/fixtures/raster/associated-emission-rec709.exr'
with tempfile.TemporaryDirectory() as directory:
 binary=Path(directory)/'oracle';flags=shlex.split(subprocess.check_output(['pkg-config','--cflags','--libs','OpenEXR'],text=True))
 subprocess.run([os.environ.get('CXX','c++'),'-std=c++17',str(source),*flags,'-o',str(binary)],check=True)
 result=json.loads(subprocess.check_output([str(binary),str(fixture)],text=True))
result['reference_version']=subprocess.check_output(['pkg-config','--modversion','OpenEXR'],text=True).strip()
result['license']='CC0 authored samples and producer'
result['source_sha256']=hashlib.sha256(source.read_bytes()).hexdigest()
result['files_sha256']={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in [fixture,Path(str(fixture)+'.native-rgba32le'),Path(str(fixture)+'.over-rgb32le')]}
result['scope']='Independent Rec709/D65 signed HDR associated-alpha samples and composition reference.'
(root/'tests/fixtures/raster/exr-associated-emission.json').write_text(json.dumps(result,indent=2)+'\n')
