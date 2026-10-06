#!/usr/bin/env python3
"""Prepare every published HPatches pair; no result-based selection or homography fitting."""
import argparse, hashlib, io, json, pathlib, re, zipfile
from PIL import Image, __version__ as pillow_version

def multiply(a,b):
    return [[sum(a[i][k]*b[k][j] for k in range(3)) for j in range(3)] for i in range(3)]

def main():
    parser=argparse.ArgumentParser();parser.add_argument('corpus',type=pathlib.Path);parser.add_argument('--max-side',type=int,default=320);args=parser.parse_args()
    if args.max_side<32:parser.error('max-side must be at least 32')
    corpus=args.corpus;inventory=json.loads((corpus/'inventory.json').read_text());archive=corpus/'hpatches-sequences-release.zip';digest=hashlib.sha256()
    with archive.open('rb') as f:
        for block in iter(lambda:f.read(1024*1024),b''):digest.update(block)
    if digest.hexdigest()!=inventory['archive_sha256']:raise ValueError('Archive hash changed')
    output=corpus/'normalized';output.mkdir(exist_ok=True);frames={};pairs=[]
    with zipfile.ZipFile(archive) as z:
        for row in inventory['sequences']:
            sequence=row['name']
            if not re.fullmatch('[iv]_[A-Za-z0-9_-]+',sequence):raise ValueError(sequence)
            (output/sequence).mkdir(exist_ok=True)
            for name in row['frames']:
                if not re.fullmatch('[1-6]\\.ppm',name):raise ValueError(name)
                member=f'hpatches-sequences-release/{sequence}/{name}';encoded=z.read(member)
                with Image.open(io.BytesIO(encoded)) as original:
                    original_size=original.size;image=original.convert('RGB')
                    scale=min(1.0,args.max_side/max(image.size));size=tuple(max(1,round(v*scale)) for v in image.size)
                    image=image.resize(size,Image.Resampling.LANCZOS)
                    target=output/sequence/(pathlib.Path(name).stem+'.png');image.save(target)
                frames[(sequence,name)]={'source_member':member,'source_sha256':hashlib.sha256(encoded).hexdigest(),'source_size':original_size,'normalized_path':str(target),'normalized_sha256':hashlib.sha256(target.read_bytes()).hexdigest(),'normalized_size':size}
            for index in range(2,7):
                member=f'hpatches-sequences-release/{sequence}/H_1_{index}';raw=z.read(member);values=list(map(float,raw.split()));assert len(values)==9
                original_h=[values[:3],values[3:6],values[6:]];left=frames[(sequence,'1.ppm')];right=frames[(sequence,f'{index}.ppm')]
                sx,sy=[old/new for old,new in zip(left['source_size'],left['normalized_size'])];tx,ty=[new/old for old,new in zip(right['source_size'],right['normalized_size'])]
                source_to_original=[[sx,0,.5*sx-.5],[0,sy,.5*sy-.5],[0,0,1]]
                original_to_target=[[tx,0,.5*tx-.5],[0,ty,.5*ty-.5],[0,0,1]]
                pairs.append({'sequence':sequence,'target_index':index,'left':left,'right':right,'original_homography':original_h,'normalized_homography':multiply(original_to_target,multiply(original_h,source_to_original)),'homography_member':member,'homography_sha256':hashlib.sha256(raw).hexdigest()})
    assert len(frames)==696 and len(pairs)==580
    result={'pillow_version':pillow_version,'archive_sha256':inventory['archive_sha256'],'max_side':args.max_side,'pairs':pairs,'required_pairs':580,'frames':696,'preprocessing':'Pillow RGB, Lanczos, half-integer resize centers','geometry_convention':'Publisher homographies interpreted in original zero-based pixel-center coordinates; validation pending','scope':'Prepared independent inputs, no library match results or confidence qualification'}
    temporary=corpus/'prepared.json.tmp';temporary.write_text(json.dumps(result,indent=2)+'\n');temporary.replace(corpus/'prepared.json')
    print('Prepared all 696 frames and 580 pairs')
if __name__=='__main__':main()
