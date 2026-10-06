"""CC0 authored pixels, independent Pillow lossless encoding and readback."""
from pathlib import Path
import hashlib, json
from PIL import Image, ImageCms, features, __version__
root=Path(__file__).resolve().parent.parent/'crates/rrrah-dedup/tests/fixtures/raster-equality'
root.mkdir(parents=True,exist_ok=True)
manifest={'license':'CC0-1.0','pillow':__version__,'color_policy':'untagged sRGB; straight alpha; invisible RGB ignored','cases':[]}
for kind,size,extensions in [('opaque',(19,13),['png','tiff','bmp','ppm','tga','webp','qoi']),('alpha',(17,11),['png','tiff','tga','webp','qoi'])]:
    pixels=[((x*31+y*7)%256,(x*11+y*47)%256,(x*83+y*19)%256,255 if kind=='opaque' else [0,1,64,128,254,255][(x+3*y)%6]) for y in range(size[1]) for x in range(size[0])]
    variants={'base':pixels,'changed':pixels.copy()}
    # One visible channel changes by one code value, preserving alpha.
    variants['changed'][-1]=((pixels[-1][0]+1)%256,*pixels[-1][1:])
    if kind=='alpha':variants['hidden']=[((r+93)%256,(g+17)%256,(b+41)%256,a) if a==0 else (r,g,b,a) for r,g,b,a in pixels]
    oracle=bytes(v for p in pixels for v in p)
    (root/f'{kind}.rgba').write_bytes(oracle)
    for variant,data in variants.items():
        image=Image.new('RGBA',size);image.putdata(data)
        for ext in extensions:
            path=root/f'{kind}-{variant}.{ext}'
            encoded=image.convert('RGB') if kind=='opaque' else image
            encoded.save(path,lossless=True,exact=True,compression='tiff_lzw',colorspace='sRGB')
            read=Image.open(path).convert('RGBA');assert read.size==size
            actual=list(read.get_flattened_data())
            # VP8L may discard RGB where alpha is zero; its visible pixels must be exact.
            assert all(a[3]==b[3] and (a[3]==0 or a[:3]==b[:3]) for a,b in zip(actual,data)),path
            manifest['cases'].append({'filename':path.name,'kind':kind,'variant':variant,'width':size[0],'height':size[1]})
# Same RGB samples explicitly declared linear are a color-policy negative.
linear=Image.open(root/'opaque-base.png').convert('RGB')
linear.save(root/'opaque-linear.qoi',colorspace='linear')
assert (root/'opaque-linear.qoi').read_bytes()[13]==1
assert (root/'opaque-base.qoi').read_bytes()[13]==0
profile=ImageCms.ImageCmsProfile(ImageCms.createProfile('sRGB')).tobytes()
linear.save(root/'opaque-tagged.png',icc_profile=profile)
read=Image.open(root/'opaque-tagged.png')
assert read.info['icc_profile']==profile
assert list(read.convert('RGBA').get_flattened_data())==list(Image.open(root/'opaque-base.png').convert('RGBA').get_flattened_data())
manifest['lcms']=features.version('littlecms2')
manifest['sha256']={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(root.iterdir()) if p.name!='manifest.json'}
(root/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(len(manifest['cases']),'independently encoded and read back fixtures')
