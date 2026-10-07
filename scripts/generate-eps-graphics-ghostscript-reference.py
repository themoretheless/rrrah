#!/usr/bin/env python3
"""Independent graphics-state numeric references. Ghostscript is test-only."""
import hashlib
import json
import subprocess
from pathlib import Path

cases = [
    [['moveto',10,20],['currentpoint']],
    [['translate',5,7],['moveto',1,2],['lineto',8,9],['currentpoint'],['pathbbox']],
    [['moveto',10,20],['scale',2,4],['currentpoint'],['pathbbox']],
    [['moveto',10,20],['translate',3,4],['currentpoint']],
    [['moveto',0,0],['curveto',10,30,20,-20,30,0],['pathbbox']],
    [['moveto',5,7],['rcurveto',10,30,20,-20,30,0],['currentpoint'],['pathbbox']],
    [['moveto',1,2],['lineto',8,9],['closepath'],['currentpoint'],['pathbbox']],
    [['moveto',1,2],['lineto',8,9],['gsave'],['moveto',100,200],['lineto',300,400],['grestore'],['currentpoint'],['pathbbox']],
    [['moveto',1,2],['lineto',8,9],['gsave'],['fill'],['grestore'],['currentpoint'],['pathbbox']],
    [['moveto',0,0],['lineto',10,20],['moveto',100,200],['pathbbox']],
    [['moveto',0,0],['moveto',100,200],['pathbbox']],
    [['moveto',1,2],['lineto',15,20],['rotate',33],['currentpoint'],['pathbbox']],
    [['rotate',25],['moveto',1,2],['lineto',15,20],['pathbbox']],
    [['scale',2,3],['moveto',1,2],['rlineto',4,5],['rmoveto',2,-1],['currentpoint']],
    [['setrgbcolor',1.5,-2,0.25],['currentrgbcolor']],
    [['setcmykcolor',0.25,0.5,0.75,1.5],['currentcmykcolor']],
    [['setlinewidth',2.5],['gsave'],['setlinewidth',8],['grestore'],['currentlinewidth']],
]
source=['currentpagedevice /HWResolution get ==', '/rrrahOracleDump {count {==} repeat} bind def']
for i,actions in enumerate(cases):
    source.extend([f'(CASE {i}\n) print','save /rrrahOracleSave exch def clear initgraphics newpath'])
    for op,*args in actions:
        source.append(' '.join([*(str(x) for x in args),op]))
    source.append('rrrahOracleDump rrrahOracleSave restore')
source.append('quit')
source=('\n'.join(source)+'\n').encode()
result=subprocess.run(['docker','run','--rm','-i','alpine:3.22.1','sh','-c',
    'apk add --no-cache ghostscript >/tmp/gs-install.log && gs --version && sha256sum /usr/bin/gs && gs -q -dNODISPLAY -dBATCH -dNOPAUSE -sDEVICE=nullpage -r1152 -'],
    input=source,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=120,check=True)
output=result.stdout.decode();Path('/tmp/rrrah-eps-graphics-ghostscript-reference.log').write_text(output)
rows=output.splitlines();version,binary_sha,resolution=rows[:3];assert resolution=='[1152.0 1152.0]',resolution;references=[];current=None
for row in rows[3:]:
    if row.startswith('CASE '):
        current={'actions':cases[int(row[5:])],'values':[]};references.append(current)
    else:
        assert current is not None
        current['values'].append(float(row))
assert len(references)==len(cases)
for case in references:case['values'].reverse()
Path('tests/fixtures/eps/graphics-ghostscript-reference.json').write_text(json.dumps({
    'license':'CC0-1.0','reference':'Independent Ghostscript graphics state numeric queries, test-only',
    'ghostscript_version':version,'binary_sha256':binary_sha.split()[0],
    'source_sha256':hashlib.sha256(source).hexdigest(),'hw_resolution':resolution,'reference_device':'nullpage, 1152dpi reduces 8-bit fixed device coordinate quantization', 'cases':references,
},indent=2)+'\n')
print(f'Generated {len(references)} independent graphics-state cases.')
