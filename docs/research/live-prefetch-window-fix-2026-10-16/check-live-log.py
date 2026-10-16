#!/usr/bin/env python3
import argparse,json,re
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('log',type=Path);a=p.parse_args();s=a.log.read_text()
assert 'LIVE_RECOVERY_FAIL' not in s and 'LIVE_RASTER_DECODE_REFUSED' not in s
expected={2:['04-target.psd','02-left.psb','01-left.psd'],3:['05-right.psb','03-selected.exr','02-left.psb'],4:['02-left.psb','04-target.psd','05-right.psb']}
actual={g:[] for g in expected}
for g,path,count in re.findall(r'LIVE_RASTER_PREFETCH_READY generation=(\d+) path=(\S+) retained=true count=(\d+)',s):
 g=int(g)
 if g in actual:
  actual[g].append(Path(path).name);assert int(count)==len(actual[g])
assert actual==expected,(actual,expected)
assert 'tier=ram swap_reads_before=0 swap_reads_after=0 foreground=true' in s
assert 'LIVE_RASTER_NAV_PASS roundtrip=true presented_generations_distinct=true same_device=true cache_required=true' in s
nav=re.findall(r'LIVE_RASTER_NAV_PRESENT phase=(\d+) path=(\S+) generation=(\d+) previous_generation=(\d+) frames=(\d+) same_device=true gpu_wait_ok=true',s)
assert nav and {row[0] for row in nav}=={'5','6'}
for phase,path,g,previous,frames in nav:
 assert (int(g),int(previous),Path(path).name)==((3,2,'04-target.psd') if phase=='5' else (4,3,'03-selected.exr'))
 # Repeated event-loop exit callbacks are not extra navigation transitions.
assert len({(row[0],row[2],row[3]) for row in nav})==2
print(json.dumps({'status':'passed','retained_neighbours_by_generation':actual,'unique_navigation_transitions':2,'cached_return_tier':'ram'}))
