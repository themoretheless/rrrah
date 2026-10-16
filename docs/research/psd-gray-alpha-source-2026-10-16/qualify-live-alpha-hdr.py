#!/usr/bin/env python3
import argparse,os,json,re,subprocess,hashlib
from pathlib import Path
parser=argparse.ArgumentParser();parser.add_argument('--binary',type=Path,required=True);parser.add_argument('--output',type=Path,required=True);args=parser.parse_args()
root=Path(__file__).resolve().parents[3];fixtures=root/'tests/fixtures/raster';args.output.mkdir(parents=True,exist_ok=False);binary=args.binary.resolve();binary_hash=hashlib.sha256(binary.read_bytes()).hexdigest()
cases=[('psd-tools-gray0.psd',400,359,'strict'),('psd-tools-gray0.psb',400,359,'strict')]
for line in (fixtures/'psd-tools-alpha-manifest.tsv').read_text().splitlines():
 if line.startswith('#'):continue
 f=line.split('\t');assert hashlib.sha256((fixtures/f[0]).read_bytes()).hexdigest()==f[5];cases.append((f[0],int(f[1]),int(f[2]),'linear-srgb'))
report={'status':'running','binary_sha256':binary_hash,'cases':[],'scope':'live native probe presentation/resize and OS HDR; not calibrated luminance/color or full viewer navigation'}
for name,w,h,policy in cases:
 source=fixtures/name;env=os.environ.copy()
 for key in ['RRRAH_HDR_RAW_SOURCE','RRRAH_HDR_MODEL_SOURCE']:env.pop(key,None)
 env.update(RRRAH_HDR_RASTER_SOURCE=str(source),RRRAH_HDR_UNTAGGED_COLOR=policy,RRRAH_HDR_SURFACE_SECONDS='3',RRRAH_HDR_RESIZE_PROBE='1',RRRAH_GPU_BACKEND='metal')
 log=args.output/(name+'.log')
 with log.open('w') as out:
  try:code=subprocess.run([str(binary)],env=env,stdout=out,stderr=subprocess.STDOUT,timeout=55).returncode
  except subprocess.TimeoutExpired:code=None
 text=log.read_text();frames=re.search(r'presentation complete: frames=(\d+) elapsed=([\d.]+)s',text);resize=re.search(r'live resize presentation: confirmed=true frames_after_resize=(\d+)',text)
 active=any('high_dynamic_range: Some(true)' in line and any(float(v)>1 for v in re.findall(r'current: Some\(([\d.]+)\)',line)) for line in text.splitlines() if line.startswith('display after present:'))
 passed=code==0 and frames and resize and int(frames[1])>=60 and float(frames[2])>=3 and int(resize[1])>=30 and active and f'native raster untagged color policy: {policy}' in text and f'native raster HDR surface: {w}x{h};' in text and 'native raster source credit after upload: 0' in text and 'presented: RGBA16Float / ExtendedSrgbLinear; no validation errors' in text and re.search(r'^adapter: .*backend: Metal[, }]',text,re.M) and hashlib.sha256(binary.read_bytes()).hexdigest()==binary_hash
 row={'fixture':name,'policy':policy,'exit_code':code,'passed':bool(passed),'frames':int(frames[1]) if frames else 0,'frames_after_resize':int(resize[1]) if resize else 0,'source_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),'log_sha256':hashlib.sha256(log.read_bytes()).hexdigest()};report['cases'].append(row);report['status']='running' if passed else 'failed';(args.output/'result.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(row),flush=True)
 if not passed:raise SystemExit(1)
report['status']='passed';report['total_frames']=sum(r['frames'] for r in report['cases']);report['total_frames_after_resize']=sum(r['frames_after_resize'] for r in report['cases']);(args.output/'result.json').write_text(json.dumps(report,indent=2)+'\n')
