#!/usr/bin/env python3
"""CC0 top-level dash controls; Ghostscript is a test-only oracle."""
import subprocess,json,hashlib,tempfile
from pathlib import Path
codes=['currentdash exch length', '[1 2] 3 setdash currentdash exch aload pop', '[] 5 setdash currentdash exch length', '[3] -1 setdash currentdash exch aload pop', '/a [1 2] def a 3 setdash a 0 9 put currentdash exch 0 get', '/a [1 2] def a 0 setdash currentdash pop a eq', '[1 2] 0 setdash currentdash pop dup 0 9 put pop currentdash pop 0 get', '[1 2] 3 setdash gsave [4 5] 6 setdash grestore currentdash exch aload pop', '[1 2] 3 setdash save /s exch def [4 5] 6 setdash s restore currentdash exch aload pop', '[1 2] 3 setdash initgraphics currentdash exch length', '[0 2] 0 setdash currentdash exch aload pop', '[2 0] 0 setdash currentdash exch aload pop', '[1 2 3] 100 setdash currentdash exch aload pop', '[0 0] 0 setdash', '[-1 2] 0 setdash', '[true 2] 0 setdash', '[1] true setdash', 'save /s exch def [1 2] 0 setdash currentdash pop s restore', '/a [1 2] def a 0 setdash save /s exch def currentdash pop s restore 0 get', '[1 2] 3 setdash save /s exch def [4] 5 setdash grestoreall currentdash exch aload pop s restore']
with tempfile.TemporaryDirectory(prefix='rrrah-dash-oracle-', dir=Path(__file__).resolve().parents[1]) as directory:
 for i,code in enumerate(codes):
  source=("errordict /handleerror { (ERROR:) print $error /errorname get == quit } put\n"+code+"\n(OK\\n) print count {dup type /booleantype eq { {1} {0} ifelse } if ==} repeat quit\n").encode()
  Path(directory,f'{i:02}.ps').write_bytes(source)
 command=['docker','run','--rm','-v',directory+':/cases:ro','alpine:3.22.1','sh','-c','apk add --no-cache ghostscript >/tmp/install.log && gs --version >&2 && sha256sum /usr/bin/gs >&2; for file in /cases/*.ps; do echo CASE; gs -q -dSAFER -dNODISPLAY -dBATCH -dNOPAUSE "$file"; echo END; done']
 r=subprocess.run(command,capture_output=True,check=True,timeout=120)
Path('/tmp/rrrah-dash-reference.stdout').write_bytes(r.stdout);Path('/tmp/rrrah-dash-reference.stderr').write_bytes(r.stderr)
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
(Path(__file__).resolve().parents[1]/'tests/fixtures/eps/dash-ghostscript-reference.json').write_text(json.dumps(report,indent=2)+'\n')
for i,c in enumerate(cases):print(i,c)

pixel_codes = [
('even','2 setlinewidth [2 2] 0 setdash 0 4 moveto 8 4 lineto stroke'),
('odd','2 setlinewidth [2] 0 setdash 0 4 moveto 8 4 lineto stroke'),
('phase','2 setlinewidth [2 2] 1 setdash 0 4 moveto 8 4 lineto stroke'),
('negative_phase','2 setlinewidth [2 2] -1 setdash 0 4 moveto 8 4 lineto stroke'),
('corner','2 setlinewidth [5 2] 0 setdash 1 2 moveto 5 2 lineto 5 6 lineto stroke'),
('closed','2 setlinewidth [6 2] 2 setdash 2 2 moveto 6 2 lineto 6 6 lineto 2 6 lineto closepath stroke'),
('affine','2 1 scale 2 setlinewidth [1 1] 0 setdash 0 4 moveto 4 4 lineto stroke'),
('cached_array','/a [2 2] def a 0 setdash a 0 6 put 2 setlinewidth 1 4 moveto 7 4 lineto stroke'),
('butt_zero','2 setlinewidth [0 2] 0 setdash 0 4 moveto 8 4 lineto stroke'),
('vertex_collinear','2 setlinewidth [4 100] 0 setdash 1 4 moveto 5 4 lineto 7 4 lineto stroke'),
('vertex_repeated','2 setlinewidth [4 100] 0 setdash 1 4 moveto 5 4 lineto 5 4 lineto 7 4 lineto stroke'),
('vertex_scaled','2 1 scale 2 setlinewidth [2 100] 0 setdash 0 4 moveto 2 4 lineto 4 4 lineto stroke'),
('butt_zero_scaled','2 1 scale 2 setlinewidth [0 1] 0 setdash 0 4 moveto 4 4 lineto stroke'),
('vertex_translated','1 0 translate 2 setlinewidth [2 100] 0 setdash 0 4 moveto 2 4 lineto 4 4 lineto stroke'),
('closed_wide','4 setlinewidth [6 2] 2 setdash 2 2 moveto 6 2 lineto 6 6 lineto 2 6 lineto closepath stroke'),
('zero_gap','2 setlinewidth [2 0] 0 setdash 0 4 moveto 6 4 lineto stroke'),
]
pixel_cases=[]
for name,code in pixel_codes:
 eps=('%!PS-Adobe-3.0 EPSF-3.0\n%%BoundingBox: 0 0 8 8\n%%EndComments\n'+code+'\nshowpage\n%%EOF\n').encode()
 command=['docker','run','--rm','-i','alpine:3.22.1','sh','-c','apk add --no-cache ghostscript >/tmp/install.log && gs --version >&2 && sha256sum /usr/bin/gs >&2 && gs -q -dSAFER -dBATCH -dNOPAUSE -sDEVICE=ppmraw -dGraphicsAlphaBits=1 -r72 -g8x8 -sOutputFile=%stdout -']
 pixels=subprocess.run(command,input=b'false setstrokeadjust\n'+eps,capture_output=True,check=True,timeout=120)
 assert report['version'] in pixels.stderr.decode().splitlines() and report['binary_sha256'] in pixels.stderr.decode()
 header,rgb=pixels.stdout.split(b'\n255\n',1)
 assert len(rgb)==192 and [line for line in header.splitlines() if not line.startswith(b'#')]==[b'P6',b'8 8']
 pixel_cases.append({'name':name,'source':code,'rgb':list(rgb),'rgb_sha256':hashlib.sha256(rgb).hexdigest()})
 if name=='cached_array':
  for y in range(8):
   for x in range(8):
    assert rgb[(y*8+x)*3:(y*8+x+1)*3]==(b'\0\0\0' if x in [1,2,5,6] and y in [3,4] else b'\xff\xff\xff')
  root=Path(__file__).resolve().parents[1]
  (root/'tests/fixtures/eps/dash-prolog.ai').write_bytes(eps)
  (root/'tests/fixtures/eps/dash-prolog.rgb').write_bytes(rgb)
report['pixel_policy']='Ghostscript false setstrokeadjust; GraphicsAlphaBits=1; native unadjusted stroke policy'
report['pixel_cases']=pixel_cases
(Path(__file__).resolve().parents[1]/'tests/fixtures/eps/dash-ghostscript-reference.json').write_text(json.dumps(report,indent=2)+'\n')
