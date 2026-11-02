"""Packed palette TIFF scanlines: odd widths and deliberately nonzero padding."""
from pathlib import Path
import struct
root=Path(__file__).resolve().parents[1]/'tests/fixtures/raster'
for bits in [1,2,4]:
    count=1<<bits
    indices=[(x+y*3)%count for y in range(3) for x in range(5)]
    payload=bytearray()
    for y in range(3):
        row=indices[y*5:y*5+5]
        for x in range(0,5,8//bits):
            value=0
            for lane in range(8//bits):
                index=row[x+lane] if x+lane<5 else count-1
                value |= index << (8-bits*(lane+1))
            payload.append(value)
    for big in [False,True]:
      for endian,magic,suffix in [('<',b'II','le'),('>',b'MM','be')]:
        pack=lambda fmt,*v:struct.pack(endian+fmt,*v)
        palette_at=(16+8+10*20+8) if big else (8+2+10*12+4)
        tags=[(256,4,1,5),(257,4,1,3),(258,3,1,bits),(259,3,1,1),(262,3,1,3),
              (273,4,1,palette_at+count*6),(277,3,1,1),(278,4,1,3),(279,4,1,len(payload)),
              (320,3,count*3,palette_at)]
        data=(magic+pack('HHHQ',43,8,0,16)+pack('Q',len(tags))) if big else (magic+pack('HIH',42,8,len(tags)))
        for tag,kind,n,value in tags:
            data+=pack('HHQ' if big else 'HHI',tag,kind,n)
            if n==1:
                field=pack('H' if kind==3 else 'I',value)
                data+=field+bytes((8 if big else 4)-len(field))
            else:data+=pack('Q' if big else 'I',value)
        data+=pack('Q' if big else 'I',0)
        for c in range(3):
            for i in range(count):data+=pack('H',[i*3911,65535-i*3911,(i*i*997)%65536][c])
        data+=payload
        (root/f'palette-packed{bits}-{"big-" if big else ""}{suffix}.tiff').write_bytes(data)
