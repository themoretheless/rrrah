#!/usr/bin/env python3
"""CC0 flatness state controls; Ghostscript is a test-only oracle."""
import subprocess,json,hashlib,tempfile
from pathlib import Path
codes=['currentflat', '-3 setflat currentflat', '0 setflat currentflat', '0.05 setflat currentflat', '0.1 setflat currentflat', '0.2 setflat currentflat', '1 setflat currentflat', '10 setflat currentflat', '100 setflat currentflat', '101 setflat currentflat', '2 setflat gsave 3 setflat grestore currentflat', '2 setflat save /s exch def 3 setflat s restore currentflat', '7 setflat initgraphics currentflat', '7 setflat initmatrix currentflat', '2 setflat gsave 3 setflat grestoreall currentflat', 'save /s exch def 3 setflat grestore currentflat s restore', '/s /setflat load def 5 s currentflat', 'true setflat', 'setflat', '/f {setflat} bind def /setflat {pop} def 5 f currentflat']
codes += ['7 setflat grestore currentflat','7 setflat grestoreall currentflat']
with tempfile.TemporaryDirectory(prefix='rrrah-flat-oracle-', dir=Path(__file__).resolve().parents[1]) as directory:
 for i,code in enumerate(codes):
  source=("errordict /handleerror { (ERROR:) print $error /errorname get == quit } put\n"+code+"\n(OK\\n) print count {dup type /booleantype eq { {1} {0} ifelse } if ==} repeat quit\n").encode()
  Path(directory,f'{i:02}.ps').write_bytes(source)
 command=['docker','run','--rm','-v',directory+':/cases:ro','alpine:3.22.1','sh','-c','apk add --no-cache ghostscript >/tmp/install.log && gs --version >&2 && sha256sum /usr/bin/gs >&2; for file in /cases/*.ps; do echo CASE; gs -q -dSAFER -dNODISPLAY -dBATCH -dNOPAUSE "$file"; echo END; done']
 r=subprocess.run(command,capture_output=True,check=True,timeout=120)
Path('/tmp/rrrah-flat-reference.stdout').write_bytes(r.stdout);Path('/tmp/rrrah-flat-reference.stderr').write_bytes(r.stderr)
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
(Path(__file__).resolve().parents[1]/'tests/fixtures/eps/flat-ghostscript-reference.json').write_text(json.dumps(report,indent=2)+'\n')
for i,c in enumerate(cases):print(i,c)

pixel_controls=[
('gsave_restore','2 setflat gsave 3 setflat grestore currentflat 2 eq'),
('vm_restore','2 setflat save /s exch def 3 setflat s restore currentflat 2 eq'),
('initgraphics_preserves','7 setflat initgraphics currentflat 7 eq'),
('grestoreall_job','2 setflat gsave 3 setflat grestoreall currentflat 1 eq'),
('clamp_low','0 setflat currentflat 0.2 eq'),
('clamp_high','101 setflat currentflat 100 eq'),
('grestore_job','7 setflat grestore currentflat 1 eq'),
('grestoreall_empty','7 setflat grestoreall currentflat 1 eq'),
]
pixel_cases=[];golden=None
for name,control in pixel_controls:
 code=control+' {0 1 0 setrgbcolor} {1 0 0 setrgbcolor} ifelse 1 1 6 6 rectfill'
 eps=('%!PS-Adobe-3.0 EPSF-3.0\n%%BoundingBox: 0 0 8 8\n%%EndComments\n'+code+'\nshowpage\n%%EOF\n').encode()
 command=['docker','run','--rm','-i','alpine:3.22.1','sh','-c','apk add --no-cache ghostscript >/tmp/install.log && gs --version >&2 && sha256sum /usr/bin/gs >&2 && gs -q -dSAFER -dBATCH -dNOPAUSE -sDEVICE=ppmraw -dGraphicsAlphaBits=1 -r72 -g8x8 -sOutputFile=%stdout -']
 pixels=subprocess.run(command,input=eps,capture_output=True,check=True,timeout=120)
 assert report['version'] in pixels.stderr.decode().splitlines() and report['binary_sha256'] in pixels.stderr.decode()
 header,rgb=pixels.stdout.split(b'\n255\n',1);assert len(rgb)==192
 for y in range(8):
  for x in range(8):
   assert rgb[(y*8+x)*3:(y*8+x+1)*3]==(b'\0\xff\0' if 1<=x<7 and 1<=y<7 else b'\xff\xff\xff')
 if golden is None:golden=rgb
 assert rgb==golden
 pixel_cases.append({'name':name,'source':code,'rgb_sha256':hashlib.sha256(rgb).hexdigest()})
report['pixel_rgb']=list(golden)
report['pixel_cases']=pixel_cases
(Path(__file__).resolve().parents[1]/'tests/fixtures/eps/flat-ghostscript-reference.json').write_text(json.dumps(report,indent=2)+'\n')
