"""Run against locally built unchanged upstream PyO3 extension; no fake binding."""
import json
from photo_dna_rs import Hash
cases=[]
for width,height in [(0,0),(0,2),(1,1),(1,2),(2,1),(2,2),(8,8)]:
    try:
        Hash.from_rgb_pixels(width,height,[(0,0,0)]*(width*height))
        outcome='ok'
    except BaseException as error:
        outcome=type(error).__name__
    cases.append({'width':width,'height':height,'outcome':outcome})
keywords=[]
for label,call in [
    ('rgb_pixels',lambda: Hash.from_rgb_pixels(width=2,height=2,pixels=[(0,0,0)]*4)),
    ('hex_string',lambda: Hash.from_hex_str(hex_string='00'*144)),
    ('byte_data',lambda: Hash.from_bytes(byte_data=bytes(144))),
]:
    try:
        call(); outcome='ok'
    except BaseException as error:
        outcome=type(error).__name__+': '+str(error)
    keywords.append({'stub_keyword':label,'outcome':outcome})
print(json.dumps({'dimension_cases':cases,'stub_keyword_cases':keywords},indent=2))
