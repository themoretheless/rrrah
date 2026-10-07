#!/usr/bin/env python3
"""Run authored scalar/procedure cases through independent Ghostscript, test-only.
Uses an ephemeral official Alpine container; no runtime dependency is introduced.
"""
import hashlib
import json
import subprocess
import struct
import sys
from pathlib import Path

def generate_binary_trigonometry():
    angles = [i * 0.5 for i in range(-720, 721)] + [1e-30, -1e-30, 1e-6, -1e-6, 360090, -360090]
    codes = [f'{angle:.9g} {op}' for angle in angles for op in ['sin', 'cos']]
    source = ('2 setobjectformat (%stdout) (w) file /out exch def\n' +
              '\n'.join(f'out {code} 0 writeobject' for code in codes) +
              '\nout flushfile quit\n').encode()
    result = subprocess.run(['docker','run','--rm','-i','alpine:3.22.1','sh','-c',
        'apk add --no-cache ghostscript >/tmp/install.log && gs --version >&2 && sha256sum /usr/bin/gs >&2 && gs -q -dNODISPLAY -dBATCH -dNOPAUSE -'],
        input=source,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=120,check=True)
    assert len(result.stdout) == len(codes) * 12
    cases = []
    for i,code in enumerate(codes):
        record = result.stdout[i*12:(i+1)*12]
        # PLRM 3.14.2: IEEE little-endian header, one real, zero tag/length.
        assert record[:8] == bytes.fromhex('81010c0002000000'), record.hex()
        cases.append({'code':code,'real_bits':struct.unpack('<I',record[8:])[0]})
    rows=result.stderr.decode().splitlines()
    version=next(r for r in rows if r.count('.')==2 and r.replace('.','').isdigit())
    binary_sha=next(r.split()[0] for r in rows if r.endswith('/usr/bin/gs'))
    Path('tests/fixtures/eps/trig-binary-ghostscript-reference.json').write_text(json.dumps({
        'license':'CC0-1.0','ghostscript_version':version,'binary_sha256':binary_sha,
        'source_sha256':hashlib.sha256(source).hexdigest(),'encoding':'writeobject IEEE little-endian real; exact f32 bits',
        'cases':cases},indent=2)+'\n')
    print(f'Generated {len(cases)} exact binary trigonometry reference values.')

if '--trig-binary' in sys.argv:
    generate_binary_trigonometry()
    raise SystemExit(0)

image = 'alpine:3.22.1'
codes = [
    '/average {add 2 div} def 40 60 average',
    '/inc {1 add} def 4 inc /plus /add load def 2 plus',
    '/x 1 def /x 9 def x /add {sub} def 10 3 add',
    'true {7} {9} ifelse false {100} if 3 {1 add} repeat',
    '{ {8} exec } exec',
    '/true false def true {1} {2} ifelse',
    '1 2 3 3 1 roll',
    '1 2 3 3 -1 roll',
    '1 2 2 copy 1 index count',
    '10 4 sub 3 mul 2 div',
    '{} dup eq {} {} eq 1 1.0 eq 2147483647 2147483648.0 eq',
    '/p { {} } def p p eq',
    'true false or 255 15 and 0 not',
    '7 /exec load dup exec',
    '/fact { dup 1 gt { dup 1 sub fact mul } { pop 1 } ifelse } def 6 fact',
    '7 0 {100 add} repeat',
    '0 500 {1 add} repeat',
    '/x /y def x',
    '-7 neg -8 abs 4 3 lt 4 4 le 5 4 gt 5 5 ge 1 2 ne',
    'true false xor true false and false not',
    '1 2 exch dup pop',
    '1 2 clear count',
    '{1} {1} eq {add} {add} eq {1} dup eq {} {} eq',
    '16777217 16777216 eq 16777217 16777216.0 eq 2147483647 2147483648.0 lt 2147483647 2147483648.0 le',
    '99 ceiling -99 floor 99 round -99 truncate',
    '3.2 ceiling -4.8 ceiling 3.2 floor -4.8 floor 6.5 round -6.5 round 3.9 truncate -3.9 truncate',
    '0.49999997 round -0.50000006 round 8388609.0 round',
    '2147483647 floor -2147483648 ceiling 1.0 round -1.0 truncate',

    '5 2 idiv -5 2 idiv 5 -2 idiv -5 -2 idiv',
    '5 3 mod -5 3 mod 5 -3 mod -5 -3 mod -2147483648 -1 mod',
    '0 sqrt 9 sqrt 16.0 sqrt',

    '0 sin 90 sin 180 sin 270 sin -90 sin 360090 sin',
    '0 cos 90 cos 180 cos 270 cos 360090 cos',

]
lines=['/rrrahOracleDump {count {dup type == ==} repeat} bind def']
for i,code in enumerate(codes):
    lines.extend([f'(CASE {i}\n) print', 'save /rrrahOracleSave exch def clear', code,
                  'rrrahOracleDump rrrahOracleSave restore'])
lines.append('quit')
source=('\n'.join(lines)+'\n').encode()
result=subprocess.run(['docker','run','--rm','-i',image,'sh','-c',
    'apk add --no-cache ghostscript >/tmp/gs-install.log && gs --version && sha256sum /usr/bin/gs && gs -q -dNODISPLAY -dBATCH -dNOPAUSE -'],
    input=source,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=120,check=True)
output=result.stdout.decode()
Path('/tmp/rrrah-eps-vm-ghostscript-reference.log').write_text(output)
rows=output.splitlines()
version,binary_sha=rows[:2]
cases=[]
current=None
for row in rows[2:]:
    if row.startswith('CASE '):
        current={'code':codes[int(row[5:])], 'rows':[]};cases.append(current)
    else:
        assert current is not None
        current['rows'].append(row)
assert len(cases)==len(codes)
for case in cases:
    rows=case.pop('rows');assert len(rows)%2==0
    values=[]
    for kind,raw in zip(rows[::2],rows[1::2]):
        if kind=='integertype': value=int(raw);label='integer'
        elif kind=='realtype': value=float(raw);label='real'
        elif kind=='booleantype': assert raw in ('true','false');value=raw=='true';label='boolean'
        elif kind=='nametype': assert raw.startswith('/');value=raw[1:];label='literal_name'
        else: raise ValueError(f'Unexpected reference type {kind}')
        values.append({'type':label,'value':value})
    case['values']=list(reversed(values))
image_id=subprocess.check_output(['docker','image','inspect','--format','{{.Id}}',image],text=True).strip()
fixture=Path('tests/fixtures/eps/vm-ghostscript-reference.json')
fixture.write_text(json.dumps({'license':'CC0-1.0','reference':'Ghostscript independent execution, test-only',
    'ghostscript_version':version,'ghostscript_binary_sha256':binary_sha.split()[0],
    'image':image,'image_id':image_id,'numeric_model':'Ghostscript default 64-bit integers; these cases do not require integer-overflow promotion',
    'source_sha256':hashlib.sha256(source).hexdigest(),'cases':cases},indent=2)+'\n')
print(f'Generated {len(cases)} independent Ghostscript cases with typed operand values.')
