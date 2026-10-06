"""Independent partial-copy negatives under declared whole-frame 90% matching.
These are authored collages, not independently captured burst photographs.
"""
from pathlib import Path
import hashlib,json
from PIL import Image,__version__
root=Path(__file__).resolve().parents[1]
source=root/'crates/rrrah-dedup/tests/fixtures/photos'
out=root/'crates/rrrah-dedup/tests/fixtures/photo-collages'
out.mkdir(parents=True,exist_ok=True)
records=[]
for left in [830,898,1084,1294]:
    for right in [830,898,1084,1294]:
        if left==right:
            continue
        a=source/f'{left}-base.png';b=source/f'{right}-base.png'
        original=Image.open(a).convert('RGB');other=Image.open(b).convert('RGB')
        w,h=original.size
        for percent in [40,70]:
            split=round(w*percent/100)
            canvas=original.copy()
            canvas.paste(other.resize((w-split,h),Image.Resampling.LANCZOS),(split,0))
            path=out/f'{left}-{right}-{percent}.png';canvas.save(path)
            records.append({'left_scene':left,'replacement_scene':right,'common_percent':percent,'common_rectangle':[0,0,split,h],'dimensions':[w,h],'file':path.name,'sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'sources':[{'file':f'../photos/{p.name}','sha256':hashlib.sha256(p.read_bytes()).hexdigest()} for p in [a,b]],'label':'partial-copy challenge; expected no whole-frame candidate under declared 90% matching policy; source errors/inconclusive fits are not successful negative classifications'})
(out/'manifest.json').write_text(json.dumps({'generator':Path(__file__).name,'pillow':__version__,'license':'CC0-1.0','scope':'24 authored collages from four pinned CC0 camera previews, preserving 40% or 70% left strip exactly and replacing the rest with another scene. No captured burst or general specificity claim.', 'cases':records},indent=2)+'\n')
shadow_source=root/'crates/rrrah-dedup/tests/fixtures/rotation/base.png'
from PIL import ImageEnhance
shadow=ImageEnhance.Brightness(Image.open(shadow_source).convert('RGB')).enhance(.1)
shadow_path=out/'shadow.png';shadow.save(shadow_path)
shadow_record={'source':'../rotation/base.png','source_sha256':hashlib.sha256(shadow_source.read_bytes()).hexdigest(),'file':'shadow.png','sha256':hashlib.sha256(shadow_path.read_bytes()).hexdigest(),'operation':'Pillow encoded brightness factor .1; independent same-frame dark-signal test, not a collage negative'}
metadata=json.loads((out/'manifest.json').read_text());metadata['shadow_fixture']=shadow_record
(out/'manifest.json').write_text(json.dumps(metadata,indent=2)+'\n')
print(len(records),'partial-copy challenge files plus one low-light fixture')
