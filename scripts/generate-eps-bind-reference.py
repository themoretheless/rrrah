#!/usr/bin/env python3
"""CC0 authored bind controls; Ghostscript is a test-only independent oracle."""
import hashlib,json,subprocess
from pathlib import Path
root=Path(__file__).resolve().parents[1]
codes=[
 '/f {add} bind def /add {sub} def 7 3 f',
 '/f {unknown} bind def /unknown 9 def f',
 '/x 1 def /f {x} bind def /x 7 def f',
 '/other {add} def /f {other} bind def /other {sub} def 7 3 f',
 '/op /add load def /f {op} bind def /op /sub load def 7 3 f',
 '/p {{add}} bind def /add {sub} def 7 3 p exec',
 '/p {{later}} bind def /later /add load def /p load bind pop /later /sub load def 7 3 p exec',
 '/p {later} bind def /later /add load def /p load bind pop /later /sub load def 7 3 p',
 '/f {add} def /alias /f load def /f load bind pop /add {sub} def 7 3 alias',
 '{} bind exec 4',
 '/p {{later}} bind def /nested p def /later /add load def /nested load bind pop /later /sub load def 7 3 nested',
 '/b /bind load def /bind {pop 19} def {add} b /f exch def /add {sub} def 7 3 f',
 '[1 2] bind 0 get',
 '/p [{add}] def p bind pop /add {sub} def 7 3 p 0 get exec',
 '/p [{later}] def p bind pop /later /add load def p bind pop /later /sub load def 7 3 p 0 get exec',
 '/a 1 array def a 0 a put a bind pop 17',
]
lines=[]
for i,code in enumerate(codes):
 lines.extend([f'(CASE {i}\\n) print','save /rrrahSave exch def clear',code,'== rrrahSave restore'])
source=('\n'.join(lines)+'\nquit\n').encode()
command=['docker','run','--rm','-i','alpine:3.22.1','sh','-c','apk add --no-cache ghostscript >/tmp/gs-install.log && gs --version >&2 && sha256sum /usr/bin/gs >&2 && gs -q -dSAFER -dNODISPLAY -dBATCH -dNOPAUSE -']
r=subprocess.run(command,input=source,capture_output=True,timeout=120,check=True)
rows=r.stdout.decode().splitlines();cases=[]
for i,code in enumerate(codes):
 assert rows[i*2]==f'CASE {i}',rows
 cases.append({'code':code,'integer':int(rows[i*2+1])})
metadata=r.stderr.decode().splitlines();version=next(x for x in metadata if x and x[0].isdigit() and len(x.split('.'))==3);digest=next(x.split()[0] for x in metadata if x.endswith('/usr/bin/gs'))
report={'license':'CC0-1.0','reference':'Ghostscript independent PostScript execution, test only','version':version,'ghostscript_sha256':digest,'image_id':subprocess.check_output(['docker','image','inspect','alpine:3.22.1','--format','{{.Id}}'],text=True).strip(),'source_sha256':hashlib.sha256(source).hexdigest(),'generator_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'cases':cases}
(root/'tests/fixtures/eps/bind-ghostscript-reference.json').write_text(json.dumps(report,indent=2)+'\n')
Path('/tmp/rrrah-eps-bind-reference.stdout').write_bytes(r.stdout);Path('/tmp/rrrah-eps-bind-reference.stderr').write_bytes(r.stderr)
print(json.dumps({'cases':len(cases),'version':version}))
eps=b'''%!PS-Adobe-3.0 EPSF-3.0
%%Creator: rrrah CC0 authored bound-prolog control
%%BoundingBox: 0 0 8 8
%%EndComments
/bdef {bind def} bind def
/m {moveto} bdef /l {lineto} bdef /h {closepath} bdef
/F {fill} bdef /red {1 0 0 setrgbcolor} bdef
/paint {newpath 1 1 m 7 1 l 7 7 l 1 7 l h red F} bind def
/fill {newpath} def
/setrgbcolor {pop pop pop 0 setgray} def
paint showpage
%%EOF
'''
(root/'tests/fixtures/eps/bind-prolog.ai').write_bytes(eps)
pixel_command=command[:-1]+['apk add --no-cache ghostscript >/tmp/gs-install.log && gs --version >&2 && sha256sum /usr/bin/gs >&2 && gs -q -dSAFER -dBATCH -dNOPAUSE -sDEVICE=ppmraw -dGraphicsAlphaBits=1 -r72 -g8x8 -sOutputFile=%stdout -']
pixels=subprocess.run(pixel_command,input=eps,capture_output=True,timeout=120,check=True)
pixel_metadata=pixels.stderr.decode().splitlines()
assert version in pixel_metadata and any(row.split()[0]==digest for row in pixel_metadata if row.endswith('/usr/bin/gs'))
header,rgb=pixels.stdout.split(b'\n255\n',1)
assert [line for line in header.splitlines() if not line.startswith(b'#')]==[b'P6',b'8 8']
assert len(rgb)==8*8*3
for y in range(8):
 for x in range(8):
  assert rgb[(y*8+x)*3:(y*8+x+1)*3]==(b'\xff\0\0' if 1<=x<7 and 1<=y<7 else b'\xff\xff\xff')
(root/'tests/fixtures/eps/bind-prolog.rgb').write_bytes(rgb)
report['pixel']={'source':'bind-prolog.ai','source_sha256':hashlib.sha256(eps).hexdigest(),'white_composite_rgb':'bind-prolog.rgb','rgb_sha256':hashlib.sha256(rgb).hexdigest(),'dimensions':[8,8],'policy':'72 dpi, GraphicsAlphaBits=1, white backdrop; integer-axis rectangle has independent analytic occupancy'}
(root/'tests/fixtures/eps/bind-ghostscript-reference.json').write_text(json.dumps(report,indent=2)+'\n')
