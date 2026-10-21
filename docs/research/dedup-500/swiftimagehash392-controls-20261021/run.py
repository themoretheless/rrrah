from pathlib import Path
import subprocess,tempfile
p=Path(__file__).parent
source=Path('.dedup-work/swiftimagehash392-20261021/Sources/SwiftImageHash/Transforms/DCT.swift').read_text()
with tempfile.TemporaryDirectory(prefix='swift392-') as folder:
    root=Path(folder); src=root/'main.swift'; binary=root/'control'
    src.write_text('import Foundation\n'+source+p.joinpath('fixture.swift').read_text())
    build=subprocess.run(['swiftc',str(src),'-o',str(binary)],capture_output=True,text=True)
    p.joinpath('build.log').write_text(build.stdout+build.stderr)
    build.check_returncode()
    run=subprocess.run([str(binary)],capture_output=True,text=True)
    p.joinpath('run.log').write_text(run.stdout+run.stderr)
    run.check_returncode()
    print(run.stdout,end='')
