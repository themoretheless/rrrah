#!/usr/bin/env python3
"""CC0 alpha fixtures; FFmpeg qualifies PNG samples, Python qualifies linear math.
This is not an external MNG composition/timing oracle.
"""
import hashlib, json, struct, subprocess, zlib
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1] / 'tests/fixtures/mng'
def chunk(k,d):
    return struct.pack('>I',len(d))+k+d+struct.pack('>I',zlib.crc32(k+d))
def linear(v):
    return v/12.92 if v <= .04045 else ((v+.055)/1.055)**2.4
cases=[]
version=subprocess.check_output(['ffmpeg','-version'],text=True).splitlines()[0]
for bits,ticks in [(8,10),(16,10),(8,0)]:
    scale=257 if bits==16 else 1
    layers=[(2,1,[33,81,129,255, 200,100,50,128]),(1,1,[192,64,96,128]),(1,1,[99,123,231,0])]
    out=b'\x8aMNG\r\n\x1a\n'+chunk(b'MHDR',struct.pack('>7I',2,1,ticks,0,0,0,457))
    canvas=[0.]*8; frames=[]; checks=[]
    name=f'rgba{bits}-alpha-'+('animated' if ticks else 'still')+'.mng'
    for index,(w,h,values) in enumerate(layers):
        values=[v*scale for v in values]
        row=bytes(values) if bits==8 else struct.pack('>'+str(len(values))+'H',*values)
        body=chunk(b'IHDR',struct.pack('>IIBBBBB',w,h,bits,6,0,0,0))+chunk(b'sRGB',b'\0')+chunk(b'IDAT',zlib.compress(b'\0'+row))+chunk(b'IEND',b'')
        plane=subprocess.check_output(['ffmpeg','-v','error','-f','image2pipe','-i','pipe:0','-frames:v','1','-f','rawvideo','-pix_fmt','rgba' if bits==8 else 'rgba64le','pipe:1'],input=b'\x89PNG\r\n\x1a\n'+body)
        decoded=list(plane) if bits==8 else list(struct.unpack('<'+str(len(plane)//2)+'H',plane))
        assert decoded==values
        checks.append({'embedded_image':index,'rgba_sha256':hashlib.sha256(plane).hexdigest()})
        out+=body
        for p in range(w*h):
            src=decoded[p*4:p*4+4]; a=src[3]/((1<<bits)-1); d=canvas[p*4+3]; alpha=a+d*(1-a)
            if a:
                for c in range(3): canvas[p*4+c]=(linear(src[c]/((1<<bits)-1))*a+canvas[p*4+c]*d*(1-a))/alpha
                canvas[p*4+3]=alpha
        if ticks: frames.append(canvas.copy())
    if not ticks: frames=[canvas.copy()]
    out+=chunk(b'MEND',b''); (ROOT/name).write_bytes(out)
    for i,f in enumerate(frames): (ROOT/f'{name}-frame-{i}.f32').write_bytes(struct.pack('<8f',*f))
    cases.append({'file':name,'sha256':hashlib.sha256(out).hexdigest(),'ticks':ticks,'linear_rgba':frames,'png_checks':checks})
# Local transparency chunks and grayscale-alpha exercise distinct PNG layouts.
for name, bits, color, row, extra, expected in [
    ('rgb8-trns', 8, 2, bytes([33,81,129,192,64,96]), chunk(b'tRNS',struct.pack('>3H',33,81,129)), [33,81,129,0,192,64,96,255]),
    ('gray8-trns', 8, 0, bytes([81,192]), chunk(b'tRNS',struct.pack('>H',81)), [81,81,81,0,192,192,192,255]),
    ('palette8-trns', 8, 3, bytes([0,1]), chunk(b'PLTE',bytes([33,81,129,192,64,96]))+chunk(b'tRNS',bytes([0,128])), [33,81,129,0,192,64,96,128]),
    ('grayalpha8', 8, 4, bytes([81,0,192,128]), b'', [81,81,81,0,192,192,192,128]),
    ('rgb16-trns', 16, 2, struct.pack('>6H',8191,22007,32769,8192,22007,32769), chunk(b'tRNS',struct.pack('>3H',8191,22007,32769)), [8191,22007,32769,0,8192,22007,32769,65535]),
    ('gray16-trns', 16, 0, struct.pack('>2H',32768,32769), chunk(b'tRNS',struct.pack('>H',32768)), [32768,32768,32768,0,32769,32769,32769,65535]),
    ('grayalpha16', 16, 4, struct.pack('>4H',22007,1,32769,32768), b'', [22007,22007,22007,1,32769,32769,32769,32768]),
]:
    name += '.mng'
    body=chunk(b'IHDR',struct.pack('>IIBBBBB',2,1,bits,color,0,0,0))+chunk(b'sRGB',b'\0')+extra+chunk(b'IDAT',zlib.compress(b'\0'+row))+chunk(b'IEND',b'')
    plane=subprocess.check_output(['ffmpeg','-v','error','-f','image2pipe','-i','pipe:0','-frames:v','1','-f','rawvideo','-pix_fmt','rgba' if bits==8 else 'rgba64le','pipe:1'],input=b'\x89PNG\r\n\x1a\n'+body)
    decoded=list(plane) if bits==8 else list(struct.unpack('<8H',plane))
    assert decoded==expected, (name,decoded,expected)
    # Transparent pixels have no initial coverage; nonzero coverage retains straight RGB.
    linear_rgba=[]
    for i in range(2):
        pixel=expected[i*4:i*4+4]
        linear_rgba.extend([linear(v/((1<<bits)-1)) if pixel[3] else 0. for v in pixel[:3]]+[pixel[3]/((1<<bits)-1)])
    out=b'\x8aMNG\r\n\x1a\n'+chunk(b'MHDR',struct.pack('>7I',2,1,0,0,0,0,457))+body+chunk(b'MEND',b'')
    (ROOT/name).write_bytes(out)
    (ROOT/f'{name}-frame-0.f32').write_bytes(struct.pack('<8f',*linear_rgba))
    cases.append({'file':name,'sha256':hashlib.sha256(out).hexdigest(),'ticks':0,'linear_rgba':[linear_rgba],'png_checks':[{'embedded_image':0,'rgba_sha256':hashlib.sha256(plane).hexdigest()}]})
for override in [False, True]:
    base=next(c for c in cases if c['file']=='palette8-trns.mng')
    source=(ROOT/base['file']).read_bytes()
    at=48; body=b''; palette=b''; trns=b''
    while at<len(source):
        n=struct.unpack('>I',source[at:at+4])[0]; end=at+n+12; kind=source[at+4:at+8]
        if kind==b'PLTE':
            palette=source[at:end]; body+=chunk(b'PLTE',b'')
        elif kind==b'tRNS':
            trns=source[at:end]
            if override: body+=chunk(b'tRNS',bytes([255,64]))
        elif kind!=b'MEND': body+=source[at:end]
        at=end
    name='palette8-global'+('-override' if override else '')+'.mng'
    out=source[:8]+chunk(b'MHDR',struct.pack('>7I',2,1,0,0,0,0,459))+palette+trns+body+chunk(b'MEND',b'')
    values=base['linear_rgba'][0].copy()
    if override: values=[linear(v/255) for v in [33,81,129]]+[1.]+[linear(v/255) for v in [192,64,96]]+[64/255]
    (ROOT/name).write_bytes(out); (ROOT/f'{name}-frame-0.f32').write_bytes(struct.pack('<8f',*values))
    cases.append({'file':name,'sha256':hashlib.sha256(out).hexdigest(),'ticks':0,'linear_rgba':[values],'png_checks':base['png_checks'],'note':'PNG source qualification reused; global palette substitution and override tested separately'})
for original in ['rgba8-alpha-animated.mng','rgba16-alpha-animated.mng','rgb8-trns.mng','rgb16-trns.mng']:
    base=next(c for c in cases if c['file']==original)
    source=(ROOT/original).read_bytes(); out=source[:8]; at=8
    while at<len(source):
        n=struct.unpack('>I',source[at:at+4])[0]; kind=source[at+4:at+8]; data=source[at+8:at+8+n]
        if kind==b'MHDR':
            data=bytearray(data); profile=struct.unpack('>I',data[24:28])[0]; data[24:28]=struct.pack('>I',profile|2)
        elif kind==b'IHDR':
            width,height,bits,color,_,_,_=struct.unpack('>IIBBBBB',data)
            channels=4 if color==6 else 3
            data=bytearray(data); data[11]=64
        elif kind==b'IDAT':
            raw=zlib.decompress(data); stride=width*channels*(bits//8); transformed=b''
            for y in range(height):
                row=raw[y*(stride+1)+1:(y+1)*(stride+1)]; assert raw[y*(stride+1)]==0
                values=list(row) if bits==8 else list(struct.unpack('>'+str(width*channels)+'H',row))
                for i in range(0,len(values),channels):
                    values[i]=(values[i]-values[i+1])% (1<<bits)
                    values[i+2]=(values[i+2]-values[i+1])% (1<<bits)
                transformed+=b'\0'+(bytes(values) if bits==8 else struct.pack('>'+str(len(values))+'H',*values))
            data=zlib.compress(transformed)
        out+=chunk(kind,data); at+=n+12
    name='intrapixel-'+original
    (ROOT/name).write_bytes(out)
    for i,values in enumerate(base['linear_rgba']): (ROOT/f'{name}-frame-{i}.f32').write_bytes(struct.pack('<8f',*values))
    cases.append(dict(base,file=name,sha256=hashlib.sha256(out).hexdigest(),note='Filter64 samples authored by modular RGB differencing of separately FFmpeg-qualified standard PNG samples; no external MNG decoder oracle'))
def paeth(a,b,c):
    p=a+b-c; distances=[abs(p-a),abs(p-b),abs(p-c)]
    return [a,b,c][distances.index(min(distances))]
for bits in [8,16]:
    maximum=(1<<bits)-1; values=[]
    for i in range(10): values.extend([(i*7019+33)%maximum,(i*3203+81)%maximum,(i*10007+129)%maximum,[maximum,maximum//2,1,0,maximum-1][i%5]])
    raw_rows=[]; differenced_rows=[]
    for y in range(5):
        row=values[y*8:(y+1)*8]; diff=row.copy()
        for i in [0,4]: diff[i]=(diff[i]-diff[i+1])%(1<<bits); diff[i+2]=(diff[i+2]-diff[i+1])%(1<<bits)
        raw_rows.append(bytes(row) if bits==8 else struct.pack('>8H',*row))
        differenced_rows.append(bytes(diff) if bits==8 else struct.pack('>8H',*diff))
    header=struct.pack('>IIBBBBB',2,5,bits,6,0,0,0)
    ordinary=chunk(b'IHDR',header)+chunk(b'sRGB',b'\0')+chunk(b'IDAT',zlib.compress(b''.join(b'\0'+r for r in raw_rows)))+chunk(b'IEND',b'')
    plane=subprocess.check_output(['ffmpeg','-v','error','-f','image2pipe','-i','pipe:0','-frames:v','1','-f','rawvideo','-pix_fmt','rgba' if bits==8 else 'rgba64le','pipe:1'],input=b'\x89PNG\r\n\x1a\n'+ordinary)
    decoded=list(plane) if bits==8 else list(struct.unpack('<40H',plane)); assert decoded==values
    filtered=b''; previous=bytes(len(differenced_rows[0])); bpp=4*(bits//8)
    for kind,row in enumerate(differenced_rows):
        encoded=[]
        for i,v in enumerate(row):
            a=row[i-bpp] if i>=bpp else 0; b=previous[i]; c=previous[i-bpp] if i>=bpp else 0
            predictor=[0,a,b,(a+b)//2,paeth(a,b,c)][kind]
            encoded.append((v-predictor)%256)
        filtered+=bytes([kind])+bytes(encoded); previous=row
    header=bytearray(header); header[11]=64
    body=chunk(b'IHDR',header)+chunk(b'sRGB',b'\0')+chunk(b'IDAT',zlib.compress(filtered))+chunk(b'IEND',b'')
    name=f'intrapixel-rgba{bits}-filters.mng'
    out=b'\x8aMNG\r\n\x1a\n'+chunk(b'MHDR',struct.pack('>7I',2,5,0,0,0,0,459))+body+chunk(b'MEND',b'')
    linear_rgba=[]
    for i in range(10):
        pixel=values[i*4:i*4+4]; linear_rgba.extend([linear(v/maximum) if pixel[3] else 0. for v in pixel[:3]]+[pixel[3]/maximum])
    (ROOT/name).write_bytes(out); (ROOT/f'{name}-frame-0.f32').write_bytes(struct.pack('<40f',*linear_rgba))
    cases.append({'file':name,'sha256':hashlib.sha256(out).hexdigest(),'ticks':0,'linear_rgba':[linear_rgba],'png_checks':[{'embedded_image':0,'rgba_sha256':hashlib.sha256(plane).hexdigest()}],'note':'All PNG row filters on intrapixel samples; FFmpeg verifies ordinary undifferenced PNG reference'})
for bits in [8,16]:
    w,h=7,6; maximum=(1<<bits)-1; values=[]
    for i in range(w*h): values.extend([(i*7019+33)%maximum,(i*3203+81)%maximum,(i*10007+129)%maximum,[maximum,maximum//2,1,0,maximum-1][i%5]])
    ordinary_raw=b''; differenced_raw=b''
    for x0,y0,dx,dy in [(0,0,8,8),(4,0,8,8),(0,4,4,8),(2,0,4,4),(0,2,2,4),(1,0,2,2),(0,1,1,2)]:
        xs=list(range(x0,w,dx))
        if not xs: continue
        for y in range(y0,h,dy):
            row=[]
            for x in xs: row.extend(values[(y*w+x)*4:(y*w+x+1)*4])
            diff=row.copy()
            for i in range(0,len(diff),4): diff[i]=(diff[i]-diff[i+1])%(1<<bits); diff[i+2]=(diff[i+2]-diff[i+1])%(1<<bits)
            ordinary_raw+=b'\0'+(bytes(row) if bits==8 else struct.pack('>'+str(len(row))+'H',*row))
            differenced_raw+=b'\0'+(bytes(diff) if bits==8 else struct.pack('>'+str(len(diff))+'H',*diff))
    header=struct.pack('>IIBBBBB',w,h,bits,6,0,0,1)
    ordinary=chunk(b'IHDR',header)+chunk(b'sRGB',b'\0')+chunk(b'IDAT',zlib.compress(ordinary_raw))+chunk(b'IEND',b'')
    plane=subprocess.check_output(['ffmpeg','-v','error','-f','image2pipe','-i','pipe:0','-frames:v','1','-f','rawvideo','-pix_fmt','rgba' if bits==8 else 'rgba64le','pipe:1'],input=b'\x89PNG\r\n\x1a\n'+ordinary)
    decoded=list(plane) if bits==8 else list(struct.unpack('<'+str(w*h*4)+'H',plane)); assert decoded==values
    header=bytearray(header); header[11]=64
    body=chunk(b'IHDR',header)+chunk(b'sRGB',b'\0')+chunk(b'IDAT',zlib.compress(differenced_raw))+chunk(b'IEND',b'')
    name=f'intrapixel-rgba{bits}-adam7.mng'
    out=b'\x8aMNG\r\n\x1a\n'+chunk(b'MHDR',struct.pack('>7I',w,h,0,0,0,0,459))+body+chunk(b'MEND',b'')
    linear_rgba=[]
    for i in range(w*h):
        pixel=values[i*4:i*4+4]; linear_rgba.extend([linear(v/maximum) if pixel[3] else 0. for v in pixel[:3]]+[pixel[3]/maximum])
    (ROOT/name).write_bytes(out); (ROOT/f'{name}-frame-0.f32').write_bytes(struct.pack('<'+str(len(linear_rgba))+'f',*linear_rgba))
    cases.append({'file':name,'sha256':hashlib.sha256(out).hexdigest(),'ticks':0,'linear_rgba':[linear_rgba],'png_checks':[{'embedded_image':0,'rgba_sha256':hashlib.sha256(plane).hexdigest()}],'note':'Adam7 intrapixel samples; FFmpeg verifies ordinary interlaced PNG reference; pass filters use zero'})
(ROOT/'alpha-manifest.json').write_text(json.dumps({'license':'CC0-1.0','png_oracle':version,'composition_oracle':'Python float64 sRGB transfer and straight-alpha source-over; no external MNG decoder proof','cases':cases},indent=2)+'\n')
