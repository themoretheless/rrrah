#!/usr/bin/env python3
"""Select a fixed external mixed-raster workload without decoding with rrrah.
Stores paths/metadata only; never copies source media. Requires Pillow for header
size filtering; unfamiliar formats retain unknown dimensions and runtime limits.
"""
from pathlib import Path
import argparse, collections, hashlib, json, subprocess, warnings
from PIL import Image
ap=argparse.ArgumentParser(description=__doc__)
ap.add_argument('root',type=Path);ap.add_argument('--output',type=Path,required=True)
ap.add_argument('--count',type=int,default=512)
args=ap.parse_args();root=args.root.resolve()
exts={'.png','.jpg','.jpeg','.gif','.webp','.bmp','.tif','.tiff','.avif','.heic','.exr','.hdr','.jxl'}
paths=subprocess.check_output(['rg','--files','--hidden','--no-ignore',str(root)],text=True).splitlines()
groups=collections.defaultdict(list);seen=set();reasons=collections.Counter();dimensions_unknown=0
Image.MAX_IMAGE_PIXELS=4*1024*1024
for name in sorted(paths):
 p=Path(name);ext=p.suffix.lower()
 if ext not in exts:continue
 if p.is_symlink() or not p.is_file() or p.stat().st_size>1024*1024:reasons['size_over_1MiB_or_not_file']+=1;continue
 dims=None
 try:
  with warnings.catch_warnings():
   warnings.simplefilter('error',Image.DecompressionBombWarning)
   with Image.open(p) as im:dims=list(im.size)
 except Image.DecompressionBombError:reasons['pillow_pixel_limit']+=1;continue
 except Image.DecompressionBombWarning:reasons['pillow_pixel_limit']+=1;continue
 except Exception:pass
 if dims and (max(dims)>1024 or min(dims)==0):reasons['dimensions_over_1024']+=1;continue
 digest=hashlib.sha256(p.read_bytes()).hexdigest()
 if digest in seen:reasons['duplicate_source_bytes']+=1;continue
 seen.add(digest)
 groups[ext].append({'path':str(p),'sha256':digest,'extension':ext,'bytes':p.stat().st_size,'pillow_dimensions':dims})
for values in groups.values():values.sort(key=lambda r:r['sha256'])
selected=[];cursor=0
while len(selected)<args.count:
 added=False
 for ext in sorted(groups):
  if cursor<len(groups[ext]):selected.append(groups[ext][cursor]);added=True
  if len(selected)==args.count:break
 if not added:break
 cursor+=1
out={'scope':'content-unique external source files, byte/header-size selected; no rrrah decode-based filtering',
     'selection_limits':{'max_source_bytes':1024*1024,'max_known_width_or_height':1024,'unknown_dimensions':'retained under runtime budget'},
     'available_unique':sum(map(len,groups.values())),'selection_exclusions':dict(reasons),
     'selected':len(selected),'extensions':dict(collections.Counter(r['extension'] for r in selected)),
     'unknown_dimensions':sum(r['pillow_dimensions'] is None for r in selected),'sources':selected}
args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps(out,indent=2)+'\n')
print(json.dumps({k:v for k,v in out.items() if k!='sources'},indent=2))
