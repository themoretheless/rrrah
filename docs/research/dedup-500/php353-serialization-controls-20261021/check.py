from pathlib import Path
import json
# Explicit model of positive 64-bit chunks only; not execution of PHP.
a=(1,0x23); b=(0x12,3)
render=lambda chunks: ''.join(format(x,'x') for x in chunks)
assert render(a)==render(b)=='123'
x=(a[0]<<64)|a[1];y=(b[0]<<64)|b[1]
assert x!=y
r={'runtime':'Python arithmetic model following non-GMP positive-chunk source path; not original PHP','chunks_a':a,'chunks_b':b,'hex_a':render(a),'hex_b':render(b),'independent_bit_distance':(x^y).bit_count(),'fixed_width_hex_a':format(x,'032x'),'fixed_width_hex_b':format(y,'032x'),'limits':['PHP interpreter absent from PATH; no GMP/GD/Imagick or PHPUnit execution.','Source-derived serialization collision for128-bit inputs on64-bit positive integer chunks; no claim about image corpus occurrence.']}
Path(__file__).with_name('result.json').write_text(json.dumps(r,indent=2)+'\n');print(json.dumps(r))
