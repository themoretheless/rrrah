#!/usr/bin/env python3
"""Build the independent CC0 C/libjxl producer and decoded sample oracle."""
from pathlib import Path
import argparse,hashlib,json,os,shlex,subprocess,tempfile
root=Path(__file__).resolve().parents[1];source=root/'scripts/jxl-animation-oracle.c'
ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--output',type=Path,default=root/'tests/fixtures/raster');args=ap.parse_args();out=args.output;out.mkdir(parents=True,exist_ok=True)
with tempfile.TemporaryDirectory() as directory:
 binary=Path(directory)/'oracle'
 flags=shlex.split(subprocess.check_output(['pkg-config','--cflags','--libs','libjxl'],text=True))
 subprocess.run([os.environ.get('CC','cc'),'-O2',str(source),*flags,'-o',str(binary)],check=True)
 metadata=json.loads(subprocess.check_output([str(binary),str(out)],text=True))
 metadata['producer_source_sha256']=hashlib.sha256(source.read_bytes()).hexdigest()
 metadata['source_sha256']=hashlib.sha256((out/'animation-finite16.jxl').read_bytes()).hexdigest()
 metadata['rgba16le_sha256']=[hashlib.sha256((out/f'animation-finite16.jxl.{i}.rgba16le').read_bytes()).hexdigest() for i in range(3)]
 metadata['scope']='CC0 authored16bit samples match libjxl C decoder; finite codestream loops and fractional TPS verified independently. No float/HDR or full viewer qualification.'
 (out/'jxl-finite16.json').write_text(json.dumps(metadata,indent=2)+'\n')
