#!/usr/bin/env python3
"""Pinned SD10 arithmetic qualification. External LibRaw is a test oracle only."""
import argparse
from contextlib import nullcontext
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile

SD14_SHA = 'c32f247eb372c3e66ad3b433518ec11f898fdc317c8432ebe025b46d4917bcf3'
SOURCE_SHA = 'ea436ba459a7478d7b338428656fae905469de269f928749f3388db2b7b0d53f'
DCRAW_SHA = 'd18d9e43a096eea04eee2148e53068f8fa45ce95395d97128b1aa37b477eab43'
ROOT = Path(__file__).resolve().parents[1]


def sha(path):
    digest = hashlib.sha256()
    with Path(path).open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def artifact_workspace(destination):
    if destination is None:
        return tempfile.TemporaryDirectory(prefix='rrrah-x3f-qualification-')
    destination = Path(destination).resolve()
    # Never overwrite an earlier qualification or another running process.
    destination.mkdir(parents=True, exist_ok=False)
    return nullcontext(str(destination))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=Path)
    parser.add_argument('--libraw-root', type=Path, required=True,
                        help='Isolated 0.22.2 source build with USE_X3FTOOLS and static libraw.a')
    parser.add_argument('--library-dir', type=Path, default=Path('/opt/homebrew/lib'))
    parser.add_argument('--target-dir', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--dcraw-source', type=Path,
                        help='Pinned independent standalone dcraw.c; enables full output and rotation checks')
    parser.add_argument('--sd14-source', type=Path,
                        help='Also qualify pinned CC0 SD14 sensor, eight WB neutrals and complete Sunlight/Auto output')
    parser.add_argument('--artifacts-dir', type=Path,
                        help='Preserve generated oracle files in a new directory; refuses an existing directory')
    parser.add_argument('--metal-readback' , action='store_true',
                        help='Require actual Metal full-frame readback against generated linear PPMs')
    args = parser.parse_args()
    if args.metal_readback and not args.dcraw_source:
        raise SystemExit('--metal-readback requires --dcraw-source')
    if args.sd14_source:
        if not args.dcraw_source:
            raise SystemExit('--sd14-source requires --dcraw-source for complete independent output')
        args.sd14_source = args.sd14_source.resolve()
        if sha(args.sd14_source) != SD14_SHA:
            raise SystemExit('Source hash does not match the pinned Sigma SD14 fixture')
    if args.dcraw_source:
        args.dcraw_source = args.dcraw_source.resolve()
        if sha(args.dcraw_source) != DCRAW_SHA:
            raise SystemExit('Independent converter source hash does not match the pinned reference')
    args.source = args.source.resolve()
    if sha(args.source) != SOURCE_SHA:
        raise SystemExit('Source hash does not match the pinned Sigma SD10 fixture')
    args.libraw_root = args.libraw_root.resolve()
    archive = args.libraw_root / 'lib/.libs/libraw.a'
    if not archive.is_file():
        raise SystemExit('Enabled isolated static LibRaw build is missing')
    logs = []

    def run(command, env=None):
        result = subprocess.run([str(x) for x in command], cwd=ROOT, env=env,
                                text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        logs.append({'command': [str(x) for x in command],
                     'exit_code': result.returncode, 'output': result.stdout})
        if result.returncode:
            raise RuntimeError(result.stdout[-6000:])
        return result.stdout

    report = {'source_sha256': SOURCE_SHA, 'status': 'running',
              'excluded_camera_checks': ['sigma_sd14_native_sensor_matches_full_independent_unpack'],
              'scope': 'Pinned SD10 sensor/CAMF and managed processing arithmetic through chroma filtering for explicit identity and legacy Foveon targets',
              'remaining': ['Independent complete converter output unless --dcraw-source is supplied',
                            'Physical display and HDR qualification', 'Other X3F variants']}
    try:
        with artifact_workspace(args.artifacts_dir) as scratch:
            scratch = Path(scratch)
            report['artifact_workspace'] = {'path': str(scratch), 'preserved': args.artifacts_dir is not None}
            for helper in ['x3f-channel-oracle', 'x3f-camf-oracle']:
                run(['c++', '-std=c++17', '-DUSE_X3FTOOLS', ROOT/'scripts'/f'{helper}.cpp',
                     f'-I{args.libraw_root}', archive, f'-L{args.library_dir}', '-llcms2', '-lz',
                     '-o', scratch/helper])
            sensor, camf = scratch/'sensor.u16le', scratch/'camf.bin'
            output = run([scratch/'x3f-channel-oracle', args.source, sensor])
            if '0.22.2-Release' not in output or 'X3FTOOLS=1' not in output:
                raise RuntimeError('Unexpected LibRaw oracle version or disabled X3F support')
            run([scratch/'x3f-camf-oracle', args.source, camf])
            for helper in ['x3f-black-row-reference', 'x3f-black-smoothing-reference', 'x3f-sensor-reference', 'x3f-badpixel-reference', 'x3f-red-reference', 'x3f-highlights-reference', 'x3f-curves-reference', 'x3f-hue-reference', 'x3f-transform-reference', 'x3f-pixel-transform-reference', 'x3f-chroma-reference']:
                run(['c++', '-std=c++17', '-O2', '-ffp-contract=off',
                     ROOT/'scripts'/f'{helper}.cpp', '-o', scratch/helper])
            black, smooth, scene = (scratch/name for name in ['black.f32le','smooth.f32le','scene.f32le'])
            run([scratch/'x3f-black-row-reference', sensor, camf, black])
            run([scratch/'x3f-black-smoothing-reference', black, smooth])
            run([scratch/'x3f-black-smoothing-reference', black, scene, sensor])
            corrected = scratch/'corrected.i32le'
            run([scratch/'x3f-sensor-reference', sensor, camf, scene, corrected])
            repaired=scratch/'repaired.i32le'
            run([scratch/'x3f-badpixel-reference', corrected, camf, repaired])
            red=scratch/'red.i32le'
            run([scratch/'x3f-red-reference', repaired, red])
            highlights=scratch/'highlights.i32le'
            run([scratch/'x3f-highlights-reference', red, camf, highlights])
            curves=scratch/'curves.bin'
            run([scratch/'x3f-curves-reference', camf, curves])
            hue=scratch/'hue.i32le'
            run([scratch/'x3f-hue-reference', highlights, curves, hue])
            wide_hue=scratch/'wide-hue.i32le'
            run([scratch/'x3f-hue-reference', hue, curves, wide_hue, 'wide'])
            transform=scratch/'transform.bin'
            run([scratch/'x3f-transform-reference', camf, transform])
            legacy_transform=scratch/'legacy-transform.bin'
            run([scratch/'x3f-transform-reference', camf, legacy_transform, 'legacy'])
            derived_curves=scratch/'derived-curves.bin'
            run([scratch/'x3f-curves-reference', camf, derived_curves, transform])
            color=scratch/'color.i32le'
            run([scratch/'x3f-pixel-transform-reference', wide_hue, curves, transform, color])
            guide=scratch/'guide.i16le'
            chroma=scratch/'chroma.i32le'
            run([scratch/'x3f-chroma-reference', color, curves, guide, chroma])
            derived_hue=scratch/'derived-hue.i32le'
            derived_wide=scratch/'derived-wide.i32le'
            derived_color=scratch/'derived-color.i32le'
            derived_guide=scratch/'derived-guide.i16le'
            derived_chroma=scratch/'derived-chroma.i32le'
            run([scratch/'x3f-hue-reference', highlights, derived_curves, derived_hue])
            run([scratch/'x3f-hue-reference', derived_hue, derived_curves, derived_wide, 'wide'])
            run([scratch/'x3f-pixel-transform-reference', derived_wide, derived_curves, transform, derived_color])
            run([scratch/'x3f-chroma-reference', derived_color, derived_curves, derived_guide, derived_chroma])
            legacy_curves=scratch/'legacy-curves.bin'
            legacy_hue=scratch/'legacy-hue.i32le'
            legacy_wide=scratch/'legacy-wide.i32le'
            legacy_color=scratch/'legacy-color.i32le'
            legacy_guide=scratch/'legacy-guide.i16le'
            legacy_frame=scratch/'legacy-frame.i32le'
            run([scratch/'x3f-curves-reference', camf, legacy_curves, legacy_transform])
            run([scratch/'x3f-hue-reference', highlights, legacy_curves, legacy_hue])
            run([scratch/'x3f-hue-reference', legacy_hue, legacy_curves, legacy_wide, 'wide'])
            run([scratch/'x3f-pixel-transform-reference', legacy_wide, legacy_curves, legacy_transform, legacy_color])
            run([scratch/'x3f-chroma-reference', legacy_color, legacy_curves, legacy_guide, legacy_frame])
            artifacts = {'legacy_frame': legacy_frame, 'legacy_guide': legacy_guide, 'legacy_color': legacy_color, 'legacy_wide': legacy_wide, 'legacy_hue': legacy_hue, 'legacy_curves': legacy_curves, 'legacy_transform': legacy_transform, 'derived_chroma': derived_chroma, 'derived_guide': derived_guide, 'derived_color': derived_color, 'derived_wide': derived_wide, 'derived_hue': derived_hue, 'derived_curves': derived_curves, 'guide': guide, 'chroma': chroma, 'color': color, 'transform': transform, 'wide_hue': wide_hue, 'hue': hue, 'curves': curves, 'highlights': highlights, 'red': red, 'repaired': repaired, 'corrected': corrected, 'sensor': sensor, 'camf': camf, 'black': black, 'smooth': smooth, 'scene': scene}
            full_markers = []
            full_env = {}
            if args.dcraw_source:
                converter = scratch/'dcraw-reference'
                run(['cc', '-O2', '-ffp-contract=off', '-DNO_JASPER', '-DNO_JPEG',
                     '-DNO_LCMS', args.dcraw_source, '-lm', '-o', converter])
                for name, rotation in [('linear-srgb', 0), ('rotation90', 90), ('rotation180', 180), ('rotation270', 270)]:
                    source = scratch/f'{name}.x3f'
                    data = bytearray(args.source.read_bytes())
                    data[36:40] = rotation.to_bytes(4, 'little')
                    source.write_bytes(data)
                    run([converter, '-4', '-W', '-o', '1', source])
                    ppm = source.with_suffix('.ppm')
                    artifacts[name] = ppm
                    variable = {0: 'LINEAR_SRGB_ORACLE', 90: 'ROTATED_ORACLE',
                                180: 'ROTATED180_ORACLE', 270: 'ROTATED270_ORACLE'}[rotation]
                    full_env['RRRAH_X3F_' + variable] = str(ppm)
                report['independent_converter_sha256'] = sha(args.dcraw_source)
                report['scope'] = 'Pinned SD10 arithmetic plus independent full linear sRGB output, public raster opening and 90/180/270-degree rotations'
                report['remaining'] = ['Other cameras, X3F encodings and white-balance modes', 'Physical display and HDR qualification']
                full_markers = ['SD10 native container-to-linear-raster exact; all leases released',
                                'SD10 header rotation 90 exact against independent full converter',
                                'SD10 header rotation 180 exact against independent full converter',
                                'SD10 header rotation 270 exact against independent full converter',
                                'SD10 public raster opening and selection refusal pass']
            report['oracle_artifacts'] = {name: {'sha256': sha(path), 'bytes': path.stat().st_size}
                                          for name, path in artifacts.items()}
            env = {key: value for key, value in os.environ.items()
                   if not key.startswith('RRRAH_X3F_')}
            env.update(full_env)
            for name, path in [('SOURCE',args.source),('ORACLE',sensor),('CAMF_ORACLE',camf),
                               ('BLACK_ORACLE',black),('SMOOTH_ORACLE',smooth),('SCENE_ORACLE',scene),('CORRECTED_ORACLE',corrected),('REPAIRED_ORACLE',repaired),('RED_ORACLE',red),('HIGHLIGHT_ORACLE',highlights),('CURVES_ORACLE',curves),('HUE_ORACLE',hue),('WIDE_HUE_ORACLE',wide_hue),('LEGACY_FRAME_ORACLE',legacy_frame),('LEGACY_TRANSFORM_ORACLE',legacy_transform),('TRANSFORM_ORACLE',transform),('COLOR_ORACLE',color),('DERIVED_CURVES_ORACLE',derived_curves),('GUIDE_ORACLE',guide),('CHROMA_ORACLE',chroma),('DERIVED_FRAME_ORACLE',derived_chroma)]:
                env[f'RRRAH_X3F_{name}'] = str(path)
            output = run(['cargo','test','-p','rrrah-decode','--lib','x3f::','--locked',
                          '--target-dir',args.target_dir,'--','--include-ignored','--nocapture',
                          '--skip','sigma_sd14_native_sensor_matches_full_independent_unpack'], env)
            match = re.search(r'test result: ok\. (\d+) passed; 0 failed; 0 ignored;', output)
            if not match or int(match[1]) < 26:
                raise RuntimeError('Required X3F checks did not all run successfully')
            for stage in ['initial black rows', 'smoothed black', 'scene black']:
                if f'SD10 {stage} max error=0' not in output:
                    raise RuntimeError(f'Missing exact arithmetic equality: {stage}')
            if 'SD10 corrected channels mismatches=0 max difference=0' not in output:
                raise RuntimeError('Missing exact corrected-channel equality')
            if 'SD10 repaired channels exact;' not in output:
                raise RuntimeError('Missing exact repaired-channel equality')
            if 'SD10 red sharpened channels exact' not in output:
                raise RuntimeError('Missing exact red-stage equality')
            if 'SD10 highlight channels exact' not in output:
                raise RuntimeError('Missing exact highlight-stage equality')
            if 'SD10 eight noise curves exact at explicit luminance=1' not in output:
                raise RuntimeError('Missing exact noise-bank equality')
            if 'SD10 first hue pass channels exact at explicit luminance=1' not in output:
                raise RuntimeError('Missing exact first hue-pass equality')
            if 'SD10 second hue pass channels exact at explicit luminance=1' not in output:
                raise RuntimeError('Missing exact second hue-pass equality')
            if 'SD10 transform coefficients and luminance exact for explicit identity target' not in output:
                raise RuntimeError('Missing exact transform arithmetic equality')
            if 'SD10 pixel transform exact for explicit identity target and luminance=1 curves' not in output:
                raise RuntimeError('Missing exact pixel transform equality')
            if 'SD10 final chroma and quarter guide exact at explicit test target/curves' not in output:
                raise RuntimeError('Missing exact final chroma equality')
            if 'SD10 eight noise curves exact at independently derived luminance' not in output:
                raise RuntimeError('Missing derived-luminance curve equality')
            if 'SD10 complete derived-luminance filter chain exact for explicit identity target' not in output:
                raise RuntimeError('Missing derived-luminance full-frame equality')
            if 'SD10 managed complete processing exact; output lease released' not in output:
                raise RuntimeError('Missing managed full processing equality')
            if 'SD10 managed processing budget failures and post-allocation cancellation release to zero' not in output:
                raise RuntimeError('Missing managed failure-path release evidence')
            if 'SD10 legacy Foveon target transform exact' not in output:
                raise RuntimeError('Missing legacy target transform equality')
            if 'SD10 managed legacy Foveon target full frame exact' not in output:
                raise RuntimeError('Missing legacy target full-frame equality')
            for marker in full_markers:
                if marker not in output:
                    raise RuntimeError(f'Missing independent full-converter evidence: {marker}')
            report.update(status='passed', tests_passed=int(match[1]))
            if args.sd14_source:
                report['status'] = 'running'
                sd14_sensor, sd14_camf = scratch/'sd14-sensor.u16le', scratch/'sd14-camf.bin'
                sensor_output = run([scratch/'x3f-channel-oracle', args.sd14_source, sd14_sensor])
                if '0.22.2-Release' not in sensor_output or 'X3FTOOLS=1' not in sensor_output:
                    raise RuntimeError('Unexpected SD14 unpack oracle version/capabilities')
                run([scratch/'x3f-camf-oracle', args.sd14_source, sd14_camf])
                neutral_helper = scratch/'x3f-neutral-reference'
                run(['c++', '-std=c++17', '-O2', '-ffp-contract=off',
                     ROOT/'scripts/x3f-neutral-reference.cpp', '-o', neutral_helper])
                neutral_folder = scratch/'sd14-neutrals'
                neutral_folder.mkdir()
                modes = ['Auto','Custom','Sunlight','Shade','Overcast','Incandescent','Fluorescent','Flash']
                sd14_artifacts = {'sensor': sd14_sensor, 'camf': sd14_camf}
                for mode in modes:
                    neutral = neutral_folder/f'{mode}.bin'
                    run([neutral_helper, sd14_camf, mode, neutral])
                    sd14_artifacts[f'neutral_{mode}'] = neutral
                sd14_source = scratch/'sd14-sunlight.x3f'
                sd14_source.write_bytes(args.sd14_source.read_bytes())
                run([converter, '-4', '-W', '-o', '1', sd14_source])
                sd14_artifacts['linear_Sunlight'] = sd14_source.with_suffix('.ppm')
                auto_source = scratch/'sd14-auto.x3f'
                data = bytearray(args.sd14_source.read_bytes())
                # Pinned source contains one Sunlight property value. Preserve
                # offsets/length and all sensor/CAMF bytes when selecting Auto.
                encoded = 'Sunlight'.encode('utf-16-le')
                positions = [m.start() for m in re.finditer(re.escape(encoded), data)]
                if len(positions) != 1:
                    raise RuntimeError('Ambiguous pinned SD14 Sunlight property value')
                position = positions[0]
                data[position:position+len(encoded)] = 'Auto\0\0\0\0'.encode('utf-16-le')
                auto_source.write_bytes(data)
                run([converter, '-4', '-W', '-o', '1', auto_source])
                sd14_artifacts['linear_Auto'] = auto_source.with_suffix('.ppm')
                sd14_artifacts['reference_Auto_input'] = auto_source
                report['sd14'] = {
                    'source_sha256': SD14_SHA, 'source_bytes': args.sd14_source.stat().st_size,
                    'license': 'CC0-1.0', 'auto_property_edit_offset': position,
                    'oracles': {name: {'sha256': sha(path), 'bytes': path.stat().st_size}
                                for name,path in sd14_artifacts.items()},
                    'test_invocations_passed': 0,
                    'scope': 'Exact sensor, eight calibrated neutrals, full selected Sunlight public raster and explicit Auto processing; malformed WB cleanup'
                }
                sd14_env = {key:value for key,value in os.environ.items() if not key.startswith('RRRAH_X3F_')}
                sd14_env.update(RRRAH_X3F_SD14_SOURCE=str(args.sd14_source),
                                RRRAH_X3F_SD14_ORACLE=str(sd14_sensor),
                                RRRAH_X3F_SD14_NEUTRAL_ORACLES=str(neutral_folder))
                for mode in ['Sunlight','Auto']:
                    sd14_env['RRRAH_X3F_SD14_WB'] = mode
                    sd14_env['RRRAH_X3F_SD14_LINEAR_ORACLE'] = str(sd14_artifacts[f'linear_{mode}'])
                    output = run(['cargo','test','-p','rrrah-decode','--lib',
                                  'sigma_sd14_native_sensor_matches_full_independent_unpack','--locked',
                                  '--target-dir',args.target_dir,'--','--ignored','--nocapture'],sd14_env)
                    required = ['test result: ok. 1 passed; 0 failed; 0 ignored;',
                                'SD14 all 14450688 sensor channels exact; offset=70',
                                f'SD14 {mode} full linear channels mismatches=0 max difference=0',
                                'SD14 missing duplicate non-ASCII unknown WB and cancellation refuse without fallback or retained memory',
                                'SD14 explicit Auto container opening succeeds; all leases released',
                                *(f'SD14 calibrated neutral {name} exact:' for name in modes)]
                    if mode == 'Sunlight':
                        required.append('SD14 public raster selected Sunlight matches complete independent output')
                    for marker in required:
                        if marker not in output:
                            raise RuntimeError(f'Missing SD14 qualification evidence: {marker}')
                    report['sd14']['test_invocations_passed'] += 1
                report['excluded_camera_checks'] = []
                report['scope'] += '; pinned SD14 sensor, eight WB neutrals, complete Sunlight public raster and Auto processing'
                report['status'] = 'passed'
            if args.metal_readback:
                report['status'] = 'running'
                cases = [('SD10', 2267, 1513, artifacts['linear-srgb'])]
                if args.sd14_source:
                    cases.append(('SD14', 2639, 1757, sd14_artifacts['linear_Sunlight']))
                report['metal_readback'] = []
                for camera,width,height,ppm in cases:
                    gpu_env = {key:value for key,value in os.environ.items()
                               if not key.startswith('RRRAH_X3F_') and not key.startswith('RRRAH_GPU_')}
                    gpu_env.update(RRRAH_GPU_BACKEND='metal',RRRAH_GPU_VENDOR='any',RRRAH_GPU_OPTIONAL='0',
                                   RRRAH_X3F_LINEAR_SRGB_ORACLE=str(ppm),
                                   RRRAH_X3F_EXPECTED_DIMENSIONS=f'{width}x{height}')
                    output = run(['cargo','test','-p','rrrah-gpu','--test','raster_readback',
                                  'qualified_sd10_linear_raster_full_frame_display_readback','--locked',
                                  '--target-dir',args.target_dir,'--','--ignored','--nocapture'],gpu_env)
                    marker = re.search(rf'X3F {width}x{height} full-frame GPU sRGB max deviation=(\d+); CPU leases released',output)
                    adapter = re.search(rf'X3F {width}x{height} raster adapter: (Metal [^\n]+)',output)
                    if not marker or not adapter or int(marker[1]) > 2 or 'test result: ok. 1 passed; 0 failed; 0 ignored;' not in output:
                        raise RuntimeError(f'Missing full-frame Metal qualification for {camera}')
                    report['metal_readback'].append({'camera':camera,'dimensions':[width,height],
                        'adapter':adapter[1],'pixels_checked':width*height,'max_deviation_rgba8':int(marker[1]),
                        'oracle_sha256':sha(ppm),'oracle_path':str(ppm) if args.artifacts_dir else None,'managed_cpu_final_used':0,
                        'scope':'Independent qualified linear fixture to offscreen SDR sRGB texture; not physical HDR'})
                report['status'] = 'passed'
    except Exception as error:
        report.update(status='failed', error=str(error))
    report['commands'] = logs
    report['helpers'] = {p.name: sha(p) for p in [Path(__file__),
                         *(ROOT/'scripts'/f'{name}.cpp' for name in ['x3f-channel-oracle','x3f-camf-oracle',
                           'x3f-black-row-reference','x3f-black-smoothing-reference','x3f-sensor-reference','x3f-badpixel-reference','x3f-red-reference','x3f-highlights-reference','x3f-curves-reference','x3f-hue-reference','x3f-transform-reference', 'x3f-pixel-transform-reference', 'x3f-chroma-reference', 'x3f-neutral-reference'])]}
    if args.metal_readback:
        report['helpers']['raster_readback.rs'] = sha(ROOT/'crates/rrrah-gpu/tests/raster_readback.rs')
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2)+'\n')
    print(f"X3F SD10: {report['status']}; report: {args.report}")
    if report['status'] != 'passed':
        raise SystemExit(1)


if __name__ == '__main__':
    main()
