#!/usr/bin/env python3
"""CC0 top-level where controls; Ghostscript is a test-only oracle."""
import subprocess,json,hashlib,tempfile
from pathlib import Path
codes=['/rrrah_missing where', '/x 1 def /x where { /x get } {99} ifelse', '/x 1 def /d 1 dict def d begin /x 2 def /x where {dup d eq exch /x get} {99} ifelse end', '/add where {systemdict eq} {false} ifelse', '/add {sub} def /add where {userdict eq} {false} ifelse', '/d 1 dict def d /add {mul} put d begin /add where {dup d eq exch /add get 7 3 3 -1 roll exec} {99} ifelse end', 'systemdict /add get 7 3 3 -1 roll exec', '/add {sub} def systemdict /add get 7 3 3 -1 roll exec', 'systemdict /add known systemdict /rrrah_missing known', 'systemdict begin /add where {systemdict eq} {false} ifelse end', '/add {sub} def systemdict begin 7 3 add end 7 3 add', 'systemdict /add 1 put', 'systemdict begin /x 1 def', 'save /s exch def /add where pop s restore systemdict eq', '/d 1 dict def d /x 1 put save /s exch def d begin /x where pop s restore d eq end', 'systemdict /rrrah_missing get', '/where /where load def /add where {systemdict eq} {false} ifelse', '/d 1 dict def d /x 1 put d /x 2 put d begin /x where {/x get} {99} ifelse end', 'currentdict userdict eq /true where {systemdict eq} {false} ifelse', 'save /s exch def systemdict begin s restore currentdict systemdict eq end']
with tempfile.TemporaryDirectory(prefix='rrrah-where-oracle-', dir=Path(__file__).resolve().parents[1]) as directory:
 for i,code in enumerate(codes):
  source=("errordict /handleerror { (ERROR:) print $error /errorname get == quit } put\n"+code+"\n(OK\\n) print count {dup type /booleantype eq { {1} {0} ifelse } if ==} repeat quit\n").encode()
  Path(directory,f'{i:02}.ps').write_bytes(source)
 command=['docker','run','--rm','-v',directory+':/cases:ro','alpine:3.22.1','sh','-c','apk add --no-cache ghostscript >/tmp/install.log && gs --version >&2 && sha256sum /usr/bin/gs >&2; for file in /cases/*.ps; do echo CASE; gs -q -dSAFER -dNODISPLAY -dBATCH -dNOPAUSE "$file"; echo END; done']
 r=subprocess.run(command,capture_output=True,check=True,timeout=120)
Path('/tmp/rrrah-where-reference.stdout').write_bytes(r.stdout);Path('/tmp/rrrah-where-reference.stderr').write_bytes(r.stderr)
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
(Path(__file__).resolve().parents[1]/'tests/fixtures/eps/where-ghostscript-reference.json').write_text(json.dumps(report,indent=2)+'\n')
for i,c in enumerate(cases):print(i,c)
eps=b'''%!PS-Adobe-3.0 EPSF-3.0
%%Creator: rrrah CC0 authored where scope control
%%BoundingBox: 0 0 8 8
%%EndComments
/palette 0 dict def palette begin /a [1 0 0] def end
/_rgb /setrgbcolor where { /setrgbcolor get } { /setrgbcolor load } ifelse def
/p {palette begin /a where { /a get } { [0 0 0] } ifelse end aload pop _rgb} def
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
(root/'tests/fixtures/eps/where-prolog.ai').write_bytes(eps)
(root/'tests/fixtures/eps/where-prolog.rgb').write_bytes(rgb)
report['pixel']={'source':'where-prolog.ai','source_sha256':hashlib.sha256(eps).hexdigest(),'rgb':'where-prolog.rgb','rgb_sha256':hashlib.sha256(rgb).hexdigest(),'dimensions':[8,8],'policy':'72dpi GraphicsAlphaBits=1 white backdrop; all 64 RGB pixels also checked analytically'}
(root/'tests/fixtures/eps/where-ghostscript-reference.json').write_text(json.dumps(report,indent=2)+'\n')
