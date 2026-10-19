#!/usr/bin/env python3
"""CC0 controlled frame-count scaling, all C-decoded samples independently checked."""
import argparse,hashlib,json,os,shlex,subprocess,tempfile
from pathlib import Path
ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--output',required=True,type=Path);ap.add_argument('--counts',nargs='+',type=int,default=[3,30,300,3000]);a=ap.parse_args();a.output.mkdir(parents=True,exist_ok=True)
root=Path(__file__).resolve().parents[1];source=root/'scripts/jxl-animation-scale-oracle.c';rows=[]
with tempfile.TemporaryDirectory() as directory:
 binary=Path(directory)/'oracle';flags=shlex.split(subprocess.check_output(['pkg-config','--cflags','--libs','libjxl'],text=True))
 subprocess.run([os.environ.get('CC','cc'),'-O2',str(source),*flags,'-o',str(binary)],check=True)
 for count in a.counts:
  row=json.loads(subprocess.check_output([str(binary),str(a.output),str(count)],text=True));name=f'animation-scale-{count}.jxl';row['name']=name;row['sha256']=hashlib.sha256((a.output/name).read_bytes()).hexdigest();row['selected_indices']=[0,count//2,count-1];row['rgba16le_sha256']={str(i):hashlib.sha256((a.output/f'{name}.{i}.rgba16le').read_bytes()).hexdigest() for i in row['selected_indices']};rows.append(row);print(name,flush=True)
(a.output/'manifest.json').write_text(json.dumps(dict(license='CC0',scope='5x3RGBA16 full replacements isolate frame-count/metadata scaling, not photographic complexity. All frames checked by libjxl C; only first/middle/last raw oracles retained.',producer_source_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),sources=rows),indent=2)+'\n')
