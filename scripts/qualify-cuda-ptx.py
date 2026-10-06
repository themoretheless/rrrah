#!/usr/bin/env python3
"""Assemble owned PTX; compiler acceptance is not NVIDIA execution evidence."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--ptxas', type=Path, required=True)
parser.add_argument('--docker-image', help='Optional pinned Linux image for a foreign-host compiler')
parser.add_argument('--report', type=Path, required=True)
args = parser.parse_args()
kernel = Path(__file__).resolve().parents[1] / 'crates/rrrah-cuda/src/exposure.ptx'
targets = ['sm_52', 'sm_61', 'sm_70', 'sm_75', 'sm_80', 'sm_86', 'sm_89',
           'sm_90', 'sm_100', 'sm_120']
report = {'scope': 'PTX assembly only; no GPU execution, driver JIT or pixel correctness',
          'kernel_sha256': hashlib.sha256(kernel.read_bytes()).hexdigest(),
          'ptxas_sha256': hashlib.sha256(args.ptxas.read_bytes()).hexdigest(),
          'targets': [], 'passed': False}
container = None
def run(command):
    result = subprocess.run([str(p) for p in command], capture_output=True, text=True, timeout=60)
    if result.returncode:
        raise RuntimeError(f'{command}: {result.stdout}{result.stderr}')
    return result.stdout + result.stderr
try:
    with tempfile.TemporaryDirectory(prefix='rrrah-cuda-assembly-') as temporary:
        output = Path(temporary)
        if args.docker_image:
            report['docker_image_id'] = run(['docker', 'image', 'inspect', args.docker_image,
                                           '--format', '{{.Id}}']).strip()
            container = run(['docker', 'create', '--network', 'none',
                             report['docker_image_id'], 'sleep', '300']).strip()
            # Copy into this owned container: no daemon/host bind-path assumptions.
            run(['docker', 'cp', args.ptxas.resolve(), container + ':/ptxas'])
            run(['docker', 'cp', kernel, container + ':/exposure.ptx'])
            run(['docker', 'start', container])
            run(['docker', 'exec', container, 'mkdir', '/output'])
            prefix = ['docker', 'exec', container, '/ptxas']
            source = '/exposure.ptx'
        else:
            prefix = [args.ptxas.resolve()]
            source = kernel
        report['compiler_version'] = run(prefix + ['--version']).strip()
        for target in targets:
            compiled = output / (target + '.cubin')
            destination = '/output/' + compiled.name if container else compiled
            log = run(prefix + ['--verbose', '--gpu-name', target, source,
                                '--output-file', destination])
            if f"Compiling entry function 'rrrah_exposure' for '{target}'" not in log:
                raise RuntimeError(f'{target}: required kernel compilation marker absent')
            if container:
                run(['docker', 'cp', container + ':' + destination, compiled])
            data = compiled.read_bytes()
            if not data.startswith(b'\x7fELF') or len(data) < 64:
                raise RuntimeError(f'{target}: missing cubin ELF output')
            report['targets'].append({'architecture': target, 'bytes': len(data),
                                      'cubin_sha256': hashlib.sha256(data).hexdigest(),
                                      'compiler_output': log, 'assembled': True})
        report['passed'] = True
except Exception as error:
    report['error'] = str(error)
finally:
    if container:
        try:
            run(['docker', 'rm', '--force', container])
        except Exception as error:
            report['cleanup_error'] = str(error)
            report['passed'] = False
    args.report.write_text(json.dumps(report, indent=2) + '\n')
if not report['passed']:
    raise SystemExit(report.get('error', report.get('cleanup_error')))
print(f'PTX assembled for {len(targets)} architectures; GPU execution remains unqualified')
