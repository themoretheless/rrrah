import struct,zlib,subprocess,json,hashlib
from pathlib import Path
folder=Path(__file__).resolve().parent;root=Path('.dedup-work/zigimage422-20261021');binary=root/'png-integrity-control';source=Path('docs/research/dedup-500/zigimage422-png-filter-controls-20261021/control.c');subprocess.run(['clang','-std=c99','-O2','-I',str(root/'include'),str(source),'-lm','-o',str(binary)],check=True)
def chunk(t,p,bad=False):return struct.pack('>I',len(p))+t+p+struct.pack('>I',(zlib.crc32(t+p)^int(bad))&0xffffffff)
rows=[]
for variant in ['valid','bad_IHDR_crc','bad_IDAT_crc','bad_IEND_crc','bad_adler_valid_crc']:
 compressed=zlib.compress(bytes([0,23,45,67]))
 if variant=='bad_adler_valid_crc':compressed=compressed[:-1]+bytes([compressed[-1]^1])
 chunks=[(b'IHDR',struct.pack('>IIBBBBB',1,1,8,2,0,0,0)),(b'IDAT',compressed),(b'IEND',b'')];data=b'\x89PNG\r\n\x1a\n'+b''.join(chunk(t,p,variant=='bad_'+t.decode()+'_crc') for t,p in chunks);f=root/'png-integrity.png';f.write_bytes(data);p=subprocess.run([str(binary),str(f)],capture_output=True,check=True);assert p.stdout==bytes([23,45,67,255]);rows.append(dict(variant=variant,loaded_rgba=list(p.stdout),fixture_sha256=hashlib.sha256(data).hexdigest(),known_invalid=variant!='valid'))
out=dict(cases=rows,scope='Actual original C PNG filename API,1x1knownRGBpixel; controlledCRCbitfliporAdlermismatchwithcorrectouterCRC. Positivepixeloracleonly; no broad malformedformat or security assessment.');(folder/'result.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
