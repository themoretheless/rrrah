#!/usr/bin/env python3
"""Run current viewer storage/GPU regression suites; preserve logs and explicit skips."""
import argparse
import hashlib
import json
import os
import sys
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target-dir', type=Path, required=True)
    parser.add_argument('--output-dir', type=Path, required=True,
                        help='New directory; existing results are never overwritten')
    parser.add_argument('--gpu-backend', default='metal' if sys.platform == 'darwin' else 'auto',
                        choices=['auto', 'metal', 'vulkan', 'dx12', 'gl'],
                        help='Required backend for hardware suites; unavailable devices fail')
    parser.add_argument('--storage-soak-seconds', type=int, default=0,
                        help='Run both delayed-I/O soak modes for this duration each; zero disables')
    parser.add_argument('--metal-policy-cycles', type=int, default=0,
                        help='Run small GPU policy cycles plus one 12 MP HDR policy/swap cycle; zero disables')
    parser.add_argument('--full-sensor-compute', action='store_true',
                        help='Include full-resolution compute qualification (about 2 GB tracked buffers)')
    parser.add_argument('--external-viewer-corpus', action='store_true',
                        help='Run external RAW/EIP/AI prefetch and EIP TTL tests; requires corpus environment paths')
    parser.add_argument('--gpr-corpus', type=Path,
                        help='Include six official GPR color oracles; filenames and SHA256 must match pinned fixtures')
    parser.add_argument('--gpr-hero9-sensor-oracle', type=Path,
                        help='Include highlight support regression; requires --gpr-corpus and pinned independent sensor.u16le')
    parser.add_argument('--gpu-only', action='store_true',
                        help='Run every ordinary rrrah-gpu suite, without app/storage suites')
    parser.add_argument('--full-camera-transport', action='store_true',
                        help='Run pinned full-camera transport; requires X3F/HERO9/Fusion environment paths')
    parser.add_argument('--srf-sensor-oracle', type=Path,
                        help='Run full F828 RGBE compute using the pinned independent sensor dump')
    args = parser.parse_args()
    if args.storage_soak_seconds and args.storage_soak_seconds < 10:
        parser.error('--storage-soak-seconds must be zero or at least 10')
    if not 0 <= args.metal_policy_cycles <= 10000:
        parser.error('--metal-policy-cycles must be between zero and 10000')
    environment = os.environ.copy()
    environment['RRRAH_GPU_OPTIONAL'] = '0'
    environment['RRRAH_GPU_BACKEND'] = args.gpu_backend
    environment['RRRAH_STRESS_SECONDS'] = str(args.storage_soak_seconds)
    environment['RRRAH_METAL_POLICY_CYCLES'] = str(args.metal_policy_cycles or 1)
    external_inputs = {}
    if args.external_viewer_corpus:
        for variable in ['RRRAH_SRF_SOURCE', 'RRRAH_EIP_APP_FIRST', 'RRRAH_EIP_APP_SECOND']:
            value = environment.get(variable)
            if not value or not Path(value).is_file():
                parser.error(f'--external-viewer-corpus requires an existing file at {variable}')
            external_inputs[variable] = str(Path(value).resolve())
        corpus = environment.get('RRRAH_AI_CORPUS')
        if not corpus or not all((Path(corpus) / name).is_file() for name in ['VectorApple.ai', 'one.ai']):
            parser.error('--external-viewer-corpus requires RRRAH_AI_CORPUS containing VectorApple.ai and one.ai')
        external_inputs['RRRAH_AI_CORPUS'] = str(Path(corpus).resolve())
        if not (ROOT / 'tests/IMG_9043.CR3').is_file():
            parser.error('--external-viewer-corpus requires tests/IMG_9043.CR3')
    if args.gpr_corpus:
        corpus = args.gpr_corpus.resolve()
        fixtures = json.loads((ROOT / 'crates/rrrah-decode/tests/fixtures/gpr-color-matrices.json').read_text())
        for fixture in fixtures:
            source = corpus / (fixture['camera'] + '.GPR')
            if not source.is_file() or hashlib.sha256(source.read_bytes()).hexdigest() != fixture['source_sha256']:
                parser.error(f'--gpr-corpus requires the pinned official source {source.name}')
            external_inputs[source.name] = {'path': str(source), 'sha256': fixture['source_sha256']}
        environment['RRRAH_GPR_CORPUS'] = str(corpus)
        environment['RRRAH_GPR_SOURCE'] = str(corpus / 'HERO9.GPR')
    if args.gpr_hero9_sensor_oracle:
        if not args.gpr_corpus:
            parser.error('--gpr-hero9-sensor-oracle requires --gpr-corpus')
        oracle = args.gpr_hero9_sensor_oracle.resolve()
        expected = '6592240ad9710edba5190fd1fd17c951cdb9f2359cedd3d2ab6728efaafcf882'
        if not oracle.is_file() or hashlib.sha256(oracle.read_bytes()).hexdigest() != expected:
            parser.error('--gpr-hero9-sensor-oracle requires the pinned independent HERO9 sensor.u16le')
        environment['RRRAH_GPR_SOURCE'] = str(args.gpr_corpus.resolve() / 'HERO9.GPR')
        environment['RRRAH_GPR_SENSOR_ORACLE'] = str(oracle)
        external_inputs['HERO9-sensor-oracle'] = {'path': str(oracle), 'sha256': expected}
    if args.full_camera_transport:
        manifest = json.loads((ROOT / 'docs/research/metal-full-camera-final-refresh-2026-10-08.json').read_text())
        candidates = {
            'Fusion-back.rgba32fle': Path(environment.get('RRRAH_FUSION_LINEAR_CORPUS', '')) / 'Fusion-back.rgba32fle',
            'Fusion-front.rgba32fle': Path(environment.get('RRRAH_FUSION_LINEAR_CORPUS', '')) / 'Fusion-front.rgba32fle',
            'HERO9.rgba32fle': Path(environment.get('RRRAH_HERO9_LINEAR_DUMP', '')),
            'linear-srgb.ppm': Path(environment.get('RRRAH_X3F_LINEAR_SRGB_ORACLE', '')),
        }
        for entry in manifest['inputs']:
            source = candidates[Path(entry['path']).name]
            if not source.is_file() or hashlib.sha256(source.read_bytes()).hexdigest() != entry['sha256']:
                parser.error(f'--full-camera-transport requires pinned input {source}')
            external_inputs[source.name] = {'path': str(source.resolve()), 'sha256': entry['sha256']}
        environment['RRRAH_X3F_EXPECTED_DIMENSIONS'] = '2267x1513'
    if args.srf_sensor_oracle:
        source = args.srf_sensor_oracle.resolve()
        expected = 'b3fc3d395950407a35d33392a97181fa51aac1afb0aa6b96d94a7eba00e95d42'
        if not source.is_file() or hashlib.sha256(source.read_bytes()).hexdigest() != expected:
            parser.error('--srf-sensor-oracle requires the pinned independent F828 sensor dump')
        environment['RRRAH_SRF_ORACLE'] = str(source)
        external_inputs['F828-sensor-oracle'] = {'path': str(source), 'sha256': expected}
    output = args.output_dir.resolve()
    output.mkdir(parents=True, exist_ok=False)
    common = ['--locked', '--offline', '--target-dir', str(args.target_dir.resolve())]
    suites = [
        ('cache', ['-p', 'rrrah-cache'], []),
        ('cuda-host', ['-p', 'rrrah-cuda'], []),
        # Full viewer coverage includes cancellation publication boundaries;
        # name-filtered prefetch/model-swap runs silently omitted those tests.
        ('viewer', ['-p', 'rrrah', '--bin', 'rrrah'], []),
        ('qoi-native', ['-p', 'rrrah-decode', '--lib', 'qoi_bounds::tests'], []),
        ('qoi-reference', ['-p', 'rrrah-decode', '--test', 'qoi_reference_corpus'], []),
        ('model-gpu', ['-p', 'rrrah', '--test', 'model_readback'], []),
        ('hdr-ttl-gpu', ['-p', 'rrrah', '--test', 'hdr_raster_swap', '--test', 'ttl_swap_metal'], []),
        ('camera-color-gpu', ['-p', 'rrrah-gpu', '--test', 'camera_color_readback'], ['--', '--nocapture']),
        ('gpu', ['-p', 'rrrah-gpu', '--lib', '--test', 'compute_readback', '--test', 'readback', '--test', 'raster_readback', '--test', 'model_readback'], []),
    ]
    if args.gpu_only:
        suites = [('gpu-all', ['-p', 'rrrah-gpu'], ['--', '--nocapture'])]
    if args.full_camera_transport:
        suites.append(('full-camera-transport', ['-p', 'rrrah-gpu', '--test', 'raster_readback'],
                       ['--', '--ignored', '--nocapture', '--test-threads=1']))
    if args.srf_sensor_oracle:
        suites.append(('full-f828-compute', ['-p', 'rrrah-gpu', '--test', 'rgbe_compute_readback',
                        'full_f828_managed_rgbe_matches_cpu_reference'],
                       ['--', '--ignored', '--nocapture']))
    if args.gpr_corpus:
        suites.append(('gpr-native-color', ['-p', 'rrrah-decode', '--test', 'gpr_color_oracle'],
                       ['--', '--ignored', '--nocapture']))
    if args.gpr_corpus:
        suites.append(('gpr-preload-app', ['-p', 'rrrah', '--bin', 'rrrah', 'gpr_preload_tests'],
                       ['--', '--ignored', '--nocapture', '--test-threads=1']))
    if args.gpr_corpus:
        suites.append(('gpr-default-quality-app', ['-p', 'rrrah', '--bin', 'rrrah', 'gpr_development_tests'],
                       ['--', '--ignored', '--nocapture', '--test-threads=1']))
        suites.append(('gpr-fusion-quality-app', ['-p', 'rrrah', '--bin', 'rrrah', 'fusion_warp_tests'],
                       ['--', '--ignored', '--nocapture']))
    if args.gpr_hero9_sensor_oracle:
        suites.append(('gpr-highlight-support', ['-p', 'rrrah-decode', '--test', 'gpr_highlight_support'],
                       ['--', '--ignored', '--nocapture']))
    if args.storage_soak_seconds:
        suites.append(('storage-soak', ['-p', 'rrrah-cache', '--test', 'sustained_swap'],
                       ['--', '--include-ignored', '--nocapture', '--test-threads=1']))
    if args.metal_policy_cycles:
        suites.append(('gpu-policy-cycles', ['-p', 'rrrah', '--test', 'ttl_swap_metal',
                        'live_count_size_and_ttl_changes_preserve_hdr_frames_through_swap'],
                       ['--', '--exact', '--include-ignored', '--nocapture']))
        # The small control does not exercise full-frame payloads or readback.
        # Run separately so its success cannot hide a missing large test.
        suites.append(('gpu-policy-large-hdr', ['-p', 'rrrah', '--test', 'ttl_swap_metal',
                        'large_hdr_live_policy_preserves_every_component_and_metal_pixel'],
                       ['--', '--exact', '--include-ignored', '--nocapture']))
    if args.full_sensor_compute:
        suites.append(('full-sensor-compute', ['-p', 'rrrah-gpu', '--test', 'compute_readback', 'full_sensor'],
                       ['--', '--include-ignored', '--nocapture', '--test-threads=1']))
    if args.external_viewer_corpus:
        suites.append(('external-prefetch', ['-p', 'rrrah', '--bin', 'rrrah', 'prefetch'],
                       ['--', '--include-ignored', '--nocapture', '--test-threads=1']))
        suites.append(('external-eip-ttl', ['-p', 'rrrah', '--test', 'ttl_swap_metal',
                        'expired_eip_lease_and_swap_restore_match_original_cr3_metal_frame'],
                       ['--', '--include-ignored', '--nocapture']))
    results = []
    for name, selectors, harness in suites:
        command = ['cargo', 'test', *selectors, *common, *harness]
        log = output / (name + '.log')
        print(f'Running {name}', flush=True)
        with log.open('w') as stream:
            process = subprocess.run(command, cwd=ROOT, stdout=stream, stderr=subprocess.STDOUT, env=environment)
        text = log.read_text()
        totals = re.findall(r'test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored;', text)
        row = {'suite': name, 'command': command, 'exit_code': process.returncode,
               'passed': sum(int(t[0]) for t in totals),
               'failed': sum(int(t[1]) for t in totals),
               'ignored': sum(int(t[2]) for t in totals),
               'ignored_tests': [line for line in text.splitlines() if ' ... ignored' in line],
               'log': str(log)}
        row['verified_execution'] = (process.returncode == 0 and row['passed'] > 0
                                     and row['failed'] == 0)
        if not row['verified_execution']:
            print(f'{name}: failed or no passing test execution was recorded', flush=True)
        results.append(row)
        report = {'gpu_backend': args.gpu_backend, 'gpu_optional': False, 'storage_soak_seconds_per_mode': args.storage_soak_seconds, 'gpu_policy_cycles': args.metal_policy_cycles, 'large_hdr_policy': bool(args.metal_policy_cycles), 'full_sensor_compute': args.full_sensor_compute, 'suites': results, 'complete_suite_execution': len(results) == len(suites),
                  'limitations': ['Passing host CUDA tests does not qualify NVIDIA hardware.',
                                  'Metal readback does not qualify physical HDR display.',
                                  'Ignored tests are unverified, not passing.',
                                  'These regression suites do not prove all 100 formats or sustained slow-disk workloads.']}
        if args.storage_soak_seconds:
            report['limitations'].append(
                'Storage soak uses delayed test I/O on the local disk; it does not qualify a physical slow disk.')
        if args.metal_policy_cycles:
            report['limitations'].append(
                'Large HDR policy checks one authored 12 MP payload cycle and offscreen SDR output, not physical HDR or a collection soak.')
        if args.full_sensor_compute:
            report['limitations'].append(
                'Full-sensor compute checks exposure/readback of authored float pixels; it does not qualify every RAW demosaic pipeline.')
        report['external_viewer_inputs'] = external_inputs
        report['all_executed_suites_verified'] = all(r['verified_execution'] for r in results)
        (output / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    return 1 if any(not r['verified_execution'] for r in results) else 0

if __name__ == '__main__':
    raise SystemExit(main())
