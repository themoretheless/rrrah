from pathlib import Path
import subprocess, os, signal, hashlib, shutil
root=Path('/Users/themoretheless/Documents/Sources/rrrah'); out=Path('/tmp/rrrah-associated-perf-20261019'); p=root/'crates/rrrah-decode/src/raster.rs'
original=p.read_bytes(); old=subprocess.check_output(['git','show','8ebce8658:crates/rrrah-decode/src/raster.rs'],cwd=root).decode()
def bounds(s):
 start=s.index('            if format == ImageFormat::OpenExr {',s.index('    let pixels = match sample_bytes'))
 end=s.index('            RasterPixels::Rgba32Float',start)
 return start,end
s=original.decode(); a,b=bounds(s); c,d=bounds(old); baseline=(s[:a]+old[c:d]+s[b:]).encode(); (out/'current-raster.rs').write_bytes(original)
def interrupted(*args): raise KeyboardInterrupt()
signal.signal(signal.SIGTERM,interrupted); signal.signal(signal.SIGINT,interrupted)
env=dict(os.environ,CARGO_TARGET_DIR='/tmp/rrrah-alpha-isolated-target-20261016')
def build(name):
 with (out/(name+'-build.log')).open('w') as log:
  subprocess.run(['cargo','+stable','build','-p','rrrah-decode','--example','raster_load_bench','--locked','--offline'],cwd=root,env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
 shutil.copyfile(Path(env['CARGO_TARGET_DIR'])/'debug/examples/raster_load_bench',out/(name+'-bench'))
try:
 p.write_bytes(baseline); build('before')
finally:
 if p.read_bytes()!=baseline: raise RuntimeError('Concurrent source modification: backup retained, do not overwrite')
 p.write_bytes(original)
assert p.read_bytes()==original
build('after')
print('Built pair; source restored',hashlib.sha256(original).hexdigest())
