"""CC0 PVR v3 ETC1 wrappers; independent Android block oracles via KTX fixture assembly."""
from pathlib import Path
import hashlib
import struct
ROOT = Path(__file__).resolve().parent.parent / 'tests/fixtures/raster'
manifest = ['# source\tmip\twidth\theight\tkind\tcolor\toracle\tsha256']
for color in [0, 1]:
    for horizontal in ['r', 'l']:
        for vertical in ['d', 'u']:
            prefix = f'ktx-etc1-le-{horizontal}{vertical}.ktx'
            ktx = (ROOT / prefix).read_bytes()
            at = 64 + struct.unpack_from('<I', ktx, 60)[0]
            payload = b''
            for level in range(4):
                size = struct.unpack_from('<I', ktx, at)[0]
                payload += ktx[at+4:at+4+size]
                at += 4 + size
            assert at == len(ktx)
            custom = b'CC0 custom metadata'
            metadata = b'PVR\x03' + struct.pack('<II', 3, 3) + bytes([horizontal == 'l', vertical == 'u', 0])
            metadata += b'CC0!' + struct.pack('<II', 42, len(custom)) + custom
            name = f'pvr-etc1-{color}-{horizontal}{vertical}.pvr'
            source = b'PVR\x03' + struct.pack('<IQ9I', 0, 6, color, 0, 5, 9, 1, 1, 1, 4, len(metadata)) + metadata + payload
            assert len(source) == 52 + len(metadata) + len(payload)
            (ROOT / name).write_bytes(source)
            digest = hashlib.sha256(source).hexdigest()
            for level in range(4):
                w, h = max(1, 9 >> level), max(1, 5 >> level)
                oracle = f'{name}.mip{level}.rgba8'
                (ROOT / oracle).write_bytes((ROOT / f'{prefix}.mip{level}.rgba8').read_bytes())
                manifest.append(f'{name}\t{level}\t{w}\t{h}\trgba8\t'+str(color)+f'\t{oracle}\t-\t{digest}')
(ROOT / 'pvr-etc1-manifest.tsv').write_text('\n'.join(manifest)+'\n')
print('8 PVR v3 ETC1 sources; 32 selected mip oracles')
