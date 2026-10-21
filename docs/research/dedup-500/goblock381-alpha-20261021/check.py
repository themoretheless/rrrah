from pathlib import Path
import json,sys
from PIL import Image
root=Path.cwd();c=root/'.dedup-work/goblock381-20261021';s=(c/'blockhash_test.go').read_text().splitlines()[173];expected=list(map(int,s.split('{',1)[1].split('}',1)[0].split(',')))
with Image.open(c/'test_assets/20170618_155330-small-alpha.png') as im:
 pixels=list(im.convert('RGBA').getdata());raw=[765 if a==0 else r+g+b for r,g,b,a in pixels];mismatches=[i for i,(x,y) in enumerate(zip(raw,expected)) if x!=y];assert not mismatches
result={'pixels':len(pixels),'expected_array_matches_unpremultiplied_RGB_sum':len(pixels),'raw_mismatches':mismatches,'pixel3600':{'rgba':pixels[3600],'expected':expected[3600],'observed_original_Go_from_previous_test':6,'independent_floor_premultiplied_sum':sum(v*pixels[3600][3]//255 for v in pixels[3600][:3])}}
assert result['pixel3600']['independent_floor_premultiplied_sum']==6
(root/'docs/research/dedup-500/goblock381-alpha-20261021/result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
