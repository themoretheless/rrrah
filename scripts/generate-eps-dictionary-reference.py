#!/usr/bin/env python3
"""CC0 top-level dictionary controls; Ghostscript is a test-only oracle."""
import subprocess,json,hashlib,tempfile
from pathlib import Path
codes=['/x 1 def /d 1 dict def d begin /x 2 def x currentdict d eq end x', '0 dict dup /x 3 put dup /y 4 put dup length exch /x get', '/d 1 dict def /alias d def d /x 1 put save /s exch def alias /x 2 put alias /y 3 put s restore d /x get d /y known', '/d 1 dict def d begin /x 1 def save end restore d /x get', 'currentdict /x 9 put x currentdict /x known', 'save /s exch def 0 dict s restore', 'save /s exch def 1 dict begin s restore', 'end', '/d 0 dict def d /a 1 put d /b 2 put d /c 3 put d length', '/x 1 def /a 1 dict def /b 1 dict def a begin /x 2 def b begin /x 3 def x end x end x', '/d 1 dict def d begin /p {1 2 add} bind def end /add {sub} def 7 3 d /p get exec', '/d 1 dict def d /p {s restore 8} put save /s exch def d /p get exec', '/d 1 dict def save /s exch def d begin s restore currentdict d eq end', '/d 1 dict def d begin save /s exch def /x 9 def s restore currentdict d eq end d /x known', '0 dict 0 dict eq', '/d 1 dict def d d eq', '1 dict /missing get', '1 dict /missing known', '/d 1 dict def /d2 d def d2 begin /x 7 def end d /x get', '/d 1 dict def d /x 1 put d /x 2 put d length d /x get']
with tempfile.TemporaryDirectory(prefix='rrrah-dictionary-oracle-', dir=Path(__file__).resolve().parents[1]) as directory:
 for i,code in enumerate(codes):
  source=("errordict /handleerror { (ERROR:) print $error /errorname get == quit } put\n"+code+"\n(OK\\n) print count {dup type /booleantype eq { {1} {0} ifelse } if ==} repeat quit\n").encode()
  Path(directory,f'{i:02}.ps').write_bytes(source)
 command=['docker','run','--rm','-v',directory+':/cases:ro','alpine:3.22.1','sh','-c','apk add --no-cache ghostscript >/tmp/install.log && gs --version >&2 && sha256sum /usr/bin/gs >&2; for file in /cases/*.ps; do echo CASE; gs -q -dSAFER -dNODISPLAY -dBATCH -dNOPAUSE "$file"; echo END; done']
 r=subprocess.run(command,capture_output=True,check=True,timeout=120)
Path('/tmp/rrrah-dictionary-reference.stdout').write_bytes(r.stdout);Path('/tmp/rrrah-dictionary-reference.stderr').write_bytes(r.stderr)
rows=r.stdout.decode().splitlines(); cases=[];at=0
for i,code in enumerate(codes):
 assert rows[at]=='CASE',rows[at:];at+=1
 status=rows[at];at+=1;values=[]
 while rows[at]!='END': values.append(float(rows[at]));at+=1
 at+=1
 cases.append({'code':code,'values':list(reversed(values))} if status=='OK' else {'code':code,'error':status.removeprefix('ERROR:/')})
source='\n'.join(codes).encode()
meta=r.stderr.decode().splitlines()
report={'generator_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'image_id':subprocess.check_output(['docker','image','inspect','alpine:3.22.1','--format','{{.Id}}'],text=True).strip(),'harness':'One top-level source file per fresh Ghostscript process; errordict handler only, no stopped/procedure wrapper','license':'CC0-1.0','reference':'Ghostscript test-only independent execution','version':next(x for x in meta if x.startswith('10.')),'binary_sha256':next(x.split()[0] for x in meta if x.endswith('/usr/bin/gs')),'source_sha256':hashlib.sha256(source).hexdigest(),'cases':cases}
(Path(__file__).resolve().parents[1]/'tests/fixtures/eps/dictionary-ghostscript-reference.json').write_text(json.dumps(report,indent=2)+'\n')
for i,c in enumerate(cases):print(i,c)
eps=b'''%!PS-Adobe-3.0 EPSF-3.0
%%Creator: rrrah CC0 authored dictionary rollback control
%%BoundingBox: 0 0 8 8
%%EndComments
/palette 0 dict def palette begin /a [1 0 0] def end /p {palette /a get aload pop setrgbcolor} def
save /outer exch def
palette begin a 0 0 put a 1 1 put end
/p load bind pop p 1 1 3 3 rectfill
save /inner exch def
0 0 1 setrgbcolor 0 5 2 2 rectfill
inner restore
gsave 0 setgray grestoreall
outer restore
p 5 1 2 3 rectfill
showpage
%%EOF
'''
pixel_command=['docker','run','--rm','-i','alpine:3.22.1','sh','-c','apk add --no-cache ghostscript >/tmp/install.log && gs --version >&2 && sha256sum /usr/bin/gs >&2 && gs -q -dSAFER -dBATCH -dNOPAUSE -sDEVICE=ppmraw -dGraphicsAlphaBits=1 -r72 -g8x8 -sOutputFile=%stdout -']
pixels=subprocess.run(pixel_command,input=eps,capture_output=True,check=True,timeout=120)
assert report['version'] in pixels.stderr.decode().splitlines()
assert report['binary_sha256'] in pixels.stderr.decode()
header,rgb=pixels.stdout.split(b'\n255\n',1)
assert [line for line in header.splitlines() if not line.startswith(b'#')]==[b'P6',b'8 8']
assert len(rgb)==8*8*3
for y in range(8):
 for x in range(8):
  expected=b'\0\xff\0' if 1<=x<4 and 4<=y<7 else b'\0\0\xff' if 0<=x<2 and 1<=y<3 else b'\xff\0\0' if 5<=x<7 and 4<=y<7 else b'\xff\xff\xff'
  assert rgb[(y*8+x)*3:(y*8+x+1)*3]==expected,(x,y)
root=Path(__file__).resolve().parents[1]
(root/'tests/fixtures/eps/dictionary-prolog.ai').write_bytes(eps)
(root/'tests/fixtures/eps/dictionary-prolog.rgb').write_bytes(rgb)
report['pixel']={'source':'dictionary-prolog.ai','source_sha256':hashlib.sha256(eps).hexdigest(),'rgb':'dictionary-prolog.rgb','rgb_sha256':hashlib.sha256(rgb).hexdigest(),'dimensions':[8,8],'policy':'72dpi GraphicsAlphaBits=1 white backdrop; all 64 RGB pixels also checked analytically'}
(root/'tests/fixtures/eps/dictionary-ghostscript-reference.json').write_text(json.dumps(report,indent=2)+'\n')
