"""Alternating optimized live-viewer comparison, same binary and authored RAW.
First-present timings are CPU request-to-presentation-submit spans, not GPU timestamps.
Existing debug qualification also waits for actual GPU completion and recovers contexts.
"""
import argparse, hashlib, json, os, pathlib, re, statistics, subprocess, time
parser=argparse.ArgumentParser()
parser.add_argument('--binary',type=pathlib.Path,required=True)
parser.add_argument('--source',type=pathlib.Path,required=True)
parser.add_argument('--output',type=pathlib.Path,required=True)
parser.add_argument('--pairs',type=int,default=3)
parser.add_argument('--algorithms',nargs='+',default=['bilinear','smooth-hue','malvar-he-cutler','directional','vng','rcd'])
args=parser.parse_args();args.output.mkdir(parents=True,exist_ok=True)
rows=[]
for algorithm in args.algorithms:
 for hdr in [False,True]:
  for pair in range(-1,args.pairs):
   # Excluded warmup then AB/BA/AB; both modes use the same process lifecycle.
   order=['fragment','compute'] if pair%2==0 else ['compute','fragment']
   for mode in order:
    env=os.environ.copy();env.pop('RRRAH_QUALIFY_RAW_SWITCH',None)
    env.update(RRRAH_QUALIFY_GPU_RECOVERY='1',RRRAH_HDR_SURFACE=str(int(hdr)),RUST_LOG='rrrah=debug,wgpu_hal=warn')
    name=f'{algorithm}-hdr{int(hdr)}-pair{pair}-{mode}'
    cmd=[str(args.binary.resolve()),str(args.source.resolve()),'--no-cache','--ram-cache-mb','0','--swap-mb','0','--cache-dir',str((args.output/'cache').resolve()),'--gpu-backend','metal','--gpu-demosaic',algorithm,'--prefetch-previous','0','--prefetch-next','0','--gpu-memory-mb','2048','--raw-gpu-mb','1024','--managed-memory-mb','2048']
    if mode=='compute':cmd+=['--gpu-raw-compute']
    start=time.monotonic();log=args.output/f'{name}.log'
    with log.open('w') as f:
     process=subprocess.Popen(cmd,env=env,stdout=f,stderr=subprocess.STDOUT)
     code=process.wait()
    contents=log.read_text()
    timings=[float(x) for x in re.findall(r'first window frame presented generation=\d+ total_ms=([0-9.]+)',contents)]
    row={'algorithm':algorithm,'hdr':hdr,'pair':pair,'warmup':pair<0,'mode':mode,'exit_code':code,'recovery_pass':'LIVE_RECOVERY_PASS' in contents,'first_present_ms':timings,'wall_seconds':time.monotonic()-start,'log':log.name}
    rows.append(row);print(json.dumps(row),flush=True)
    (args.output/'runs.json').write_text(json.dumps(rows,indent=2)+'\n')
    if code!=0 or not row['recovery_pass'] or len(timings)!=2:
     raise SystemExit('Qualification failed; preserve logs and inspect before continuing.')
summary=[]
for algorithm in args.algorithms:
 for hdr in [False,True]:
  modes={}
  for mode in ['fragment','compute']:
   selected=[r for r in rows if not r['warmup'] and r['algorithm']==algorithm and r['hdr']==hdr and r['mode']==mode]
   modes[mode]={'initial_present_ms':[r['first_present_ms'][0] for r in selected],'recovery_present_ms':[r['first_present_ms'][1] for r in selected]}
   for stage in ['initial','recovery']:
    modes[mode][f'{stage}_median_ms']=statistics.median(modes[mode][f'{stage}_present_ms'])
  summary.append({'algorithm':algorithm,'hdr':hdr,'modes':modes,'initial_compute_over_fragment':modes['compute']['initial_median_ms']/modes['fragment']['initial_median_ms']})
result={'binary':str(args.binary.resolve()),'binary_sha256':hashlib.sha256(args.binary.read_bytes()).hexdigest(),'source_sha256':hashlib.sha256(args.source.read_bytes()).hexdigest(),'pairs':args.pairs,'summary':summary,'limitations':['authored native DNG, not camera corpus','CPU first-present submission timing, not GPU frame throughput or display latency','release optimization with debug assertions enabled for qualification','no physical HDR calibration or NVIDIA evidence','per-frame fidelity between fragment area sampling and full-resolution compute resampling is a separate gate']}
(args.output/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result),flush=True)
