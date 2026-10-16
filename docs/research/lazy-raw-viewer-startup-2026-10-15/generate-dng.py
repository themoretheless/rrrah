"""Authored tiny sensor control based on dng_backend color_fixture tag layout."""
from pathlib import Path
import struct,hashlib,json
folder=Path('/tmp/rrrah-lazy-raw-authored-dng');folder.mkdir(exist_ok=True)
b=bytearray(1024);b[:8]=b'II*\0\x08\0\0\0'
e=[(256,4,1,16),(257,4,1,16),(258,3,1,16),(259,3,1,1),(262,3,1,32803),(273,4,1,512),(274,3,1,1),(277,3,1,1),(278,4,1,16),(279,4,1,512),(33421,3,2,0x00020002),(33422,1,4,0x02010100),(50706,1,4,0x00000401),(50708,2,5,300),(50717,4,1,65535),(50721,10,9,320),(50728,5,3,400),(50778,3,1,21)]
b[300:305]=b'TEST\0'
for i,n in enumerate([6000,-1000,-1000,-5000,15000,1000,-1000,2000,6000]):struct.pack_into('<ii',b,320+i*8,n,10000)
for i,(n,d)in enumerate([(1,2),(1,1),(1,4)]):struct.pack_into('<II',b,400+i*8,n,d)
struct.pack_into('<H',b,8,len(e))
for i,entry in enumerate(sorted(e)):struct.pack_into('<HHII',b,10+i*12,*entry)
for i in range(256):struct.pack_into('<H',b,512+i*2,1024+i*64)
p=folder/'authored.dng';p.write_bytes(b)
Path(__file__).with_name('authored-dng.json').write_text(json.dumps({'fixture':str(p),'sha256':hashlib.sha256(b).hexdigest(),'dimensions':[16,16],'scope':'authored native sensor route/startup control; not camera-produced DNG fidelity'},indent=2)+'\n')
