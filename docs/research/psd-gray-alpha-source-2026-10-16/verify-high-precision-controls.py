#!/usr/bin/env python3
"""Stdlib container/PackBits/f64 check independent of the fixture producer."""
from pathlib import Path
import json,struct,hashlib,math
from fractions import Fraction
folder=Path(__file__).resolve().parent/'high-precision-controls'
rows=[]
app_rows=[]
for record in json.loads((folder/'manifest.json').read_text()):
    p=folder/record['fixture'];b=p.read_bytes()
    assert hashlib.sha256(b).hexdigest()==record['source_sha256']
    version,channels,h,w,depth,mode=struct.unpack('>H6xHIIHH',b[4:26])
    assert (version,depth)==(record['version'],record['depth'])
    assert mode in (1,3) and (w,h)==(4,1)
    at=26
    for section in range(3):
        size=8 if version==2 and section==2 else 4
        length=int.from_bytes(b[at:at+size],'big');at+=size+length
    assert b[at:at+2]==b'\0\1';at+=2;size=4 if version==2 else 2
    lengths=[int.from_bytes(b[at+i*size:at+(i+1)*size],'big') for i in range(channels*h)]
    at+=size*len(lengths);raw=bytearray()
    for length in lengths:
        encoded=b[at:at+length];at+=length;i=0;row=bytearray()
        while i<len(encoded):
            code=encoded[i];i+=1
            if code<128:row+=encoded[i:i+code+1];i+=code+1
            elif code>128:row+=bytes([encoded[i]])*(257-code);i+=1
        assert len(row)==w*(depth//8);raw+=row
    assert at==len(b)
    values=struct.unpack('>'+str(w*h*channels)+('H' if depth==16 else 'f'),raw)
    stored_values=values
    if depth==16:values=[v/65535 for v in values]
    expected=(folder/(record['fixture']+'.f32le')).read_bytes()
    reference=struct.unpack('<'+str(len(expected)//4)+'f',expected)
    assert hashlib.sha256(expected).hexdigest()==record['oracle_sha256']
    maximum=0.
    for pixel in range(w*h):
        alpha=values[(channels-1)*w*h+pixel]
        assert abs(alpha-reference[pixel*channels+channels-1])<=1e-7
        for channel in range(channels-1):
            value=values[channel*w*h+pixel]
            straight=(value+alpha-1)/alpha if alpha>0 else value
            error=abs(straight-reference[pixel*channels+channel]);maximum=max(maximum,error)
            assert math.isfinite(straight) and error<=2e-6
    native=bytearray()
    for pixel in range(w*h):
        alpha=stored_values[(channels-1)*w*h+pixel]
        for c in range(4):
            channel=channels-1 if c==3 else 0 if channels==2 else c
            value=stored_values[channel*w*h+pixel]
            if c<3 and alpha>0:
                if depth==16:
                    rational=Fraction((value+alpha-65535)*65535,alpha)
                    value=min(65535,max(0,rational.numerator//rational.denominator))
                else:value=(value+alpha-1)/alpha
            native+=struct.pack('<H',value) if depth==16 else struct.pack('<f',value)
    name='psd-tools-alpha-'+record['fixture']
    extension='.rgba16le' if depth==16 else '.rgba32f'
    fixture_dir=folder.parents[3]/'tests/fixtures/raster'
    (fixture_dir/(name+extension)).write_bytes(native)
    app_rows.append('\t'.join([name,str(w),str(h),str(depth),name+extension,record['source_sha256']]))
    rows.append({'fixture':record['fixture'],'max_stdlib_f64_vs_psdtools_f32_error':maximum,'bound':2e-6,'alpha_matches':True})
(folder/'independent-f64-check.json').write_text(json.dumps(rows,indent=2)+'\n')
print(json.dumps({'cases':len(rows),'max_error':max(r['max_stdlib_f64_vs_psdtools_f32_error'] for r in rows),'native_support_qualified':False}))

(fixture_dir/'psd-tools-alpha-manifest.tsv').write_text('# source\twidth\theight\tdepth\toracle\tsource_sha256\n'+'\n'.join(app_rows)+'\n')
