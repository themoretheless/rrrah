from pathlib import Path
from xdis.disasm import disassemble_file
root=Path('.dedup-work/fourhash424-20261021')
out=Path('docs/research/dedup-500/fourhash424-bytecode-review-20261021')
for p in sorted(root.rglob('*.pyc')):
    with (out/(p.name+'.dis.txt')).open('w') as stream:
        disassemble_file(str(p),outstream=stream)
# Parse/disassemble only; never import or execute the historic bytecode.
