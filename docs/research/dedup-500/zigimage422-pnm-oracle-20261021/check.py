import json,sys
from pathlib import Path
observed=json.loads(Path('docs/research/dedup-500/zigimage422-depth-controls-20261021/result.json').read_text())
a=bytes.fromhex('120034005600');b=bytes.fromhex('12ff34ff56ff')
decode=lambda data:[int.from_bytes(data[i:i+2],'big') for i in range(0,len(data),2)]
expected_a=decode(a);expected_b=decode(b)
assert expected_a==[4608,13312,22016];assert expected_b==[4863,13567,22271]
assert observed['decoded16_a']!=expected_a and observed['decoded16_b']!=expected_b
for payload,key in [(a,'decoded16_a'),(b,'decoded16_b')]:assert observed[key]==[int.from_bytes(payload[i:i+2],sys.byteorder) for i in range(0,len(payload),2)]
result=dict(specification='https://netpbm.sourceforge.net/doc/ppm.html',rule='Two-byte PPM samples store most significant byte first.',expected16_a=expected_a,expected16_b=expected_b,observed=observed,host_byteorder=sys.byteorder,diagnosis='Raw PNM bytes copied by stbi__getn into allocation; uint16 interpretation follows host order with no byte swap in stbi__pnm_load. convert16to8 uses decoded value >>8.',scope='Independent explicit big-endian byte oracle against recorded actual C loader output; pinned PNM function reviewed. No full Zig end-to-end or all PNM variants qualification.')
Path(__file__).with_name('result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
