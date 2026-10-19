from pathlib import Path
import subprocess,os,json,time
root=Path('/Users/themoretheless/Documents/Sources/rrrah'); work=Path('/tmp/rrrah-live-endurance-20261019');work.mkdir(exist_ok=True)
for name,source in [('01-emission.exr','tests/fixtures/raster/associated-emission-rec709.exr'),('02-gray.psd','tests/fixtures/raster/psd-tools-gray0.psd')]:
 p=work/name
 if not p.exists(): p.symlink_to(root/source)
env=dict(os.environ,RRRAH_QUALIFY_GPU_RECOVERY='1',RRRAH_QUALIFY_RASTER_SWITCH=str(work/'02-gray.psd'),RRRAH_QUALIFY_RASTER_REQUIRE_CACHE='1',RRRAH_QUALIFY_NAV_ROUNDS='300')
cmd=['/tmp/rrrah-alpha-isolated-target-20261016/debug/rrrah','--gpu-backend','metal','--managed-memory-mb','64','--raster-gpu-mb','16','--raster-cache-mb','8','--raster-cache-count','1','--raster-swap-mb','16','--raster-swap-count','4','--raster-swap-restore-mb','8','--prefetch-previous','0','--prefetch-next','0',str(work/'01-emission.exr')]
t=time.monotonic()
with (work/'live.log').open('w') as log:
 try:
  r=subprocess.run(cmd,env=env,stdout=log,stderr=subprocess.STDOUT,timeout=240); code=r.returncode
 except subprocess.TimeoutExpired: code='watchdog_timeout'
result={'exit':code,'wall_seconds':time.monotonic()-t,'command':cmd,'rounds_requested':300}
(work/'launch.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
