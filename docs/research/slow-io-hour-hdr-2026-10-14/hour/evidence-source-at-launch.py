#!/usr/bin/env python3
"""Run current viewer storage/GPU regression suites; preserve logs and explicit skips."""
import argparse
import hashlib
import json
import math
import os
import sys
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def collection_evidence(text, seconds, full_hdr, passed, ignored):
    """Verify requested duration, complete frame counts and final accounting."""
    output = (4000, 3000) if full_hdr else (63, 65)
    seeds = re.findall(
        r'^(?:test long_lived_large_collection_cache_preserves_swap_and_metal_frames \.\.\. )?long-lived collection adapter: Metal .+; duration=(\d+)s; frames=8; payload=192000000; hdr_output=\[(\d+), (\d+)\]$',
        text, re.MULTILINE)
    completed = re.findall(
        r'^long-lived collection complete: cycles=(\d+); elapsed=(\d+(?:\.\d+)?)s; reads=(\d+); writes=8; root=0; queue=0$',
        text, re.MULTILINE)
    frames = re.findall(
        r'^parallel collection: accelerated=(\d+); mid_batch_cancelled=(\d+); exact_sdr_full_frames=(\d+); exact_hdr_half_frames=(\d+); hdr_output=\[(\d+), (\d+)\]; root=0; queue=0$',
        text, re.MULTILINE)
    if passed != 1 or ignored != 0 or any(len(rows) != 1 for rows in (seeds, completed, frames)):
        return False
    duration, width, height = map(int, seeds[0])
    cycles, elapsed, reads = completed[0]
    cycles, reads = int(cycles), int(reads)
    accelerated, cancelled, sdr, hdr, hdr_width, hdr_height = map(int, frames[0])
    return (duration == seconds and (width, height) == output
            and float(elapsed) >= seconds and cycles >= 8 and reads == cycles
            and sdr == hdr == cycles and (hdr_width, hdr_height) == output
            and accelerated == cancelled and accelerated in (0, cycles))


def signed_upload_recovery_evidence(text, passed, ignored):
    """Reject older SDR-only/0..1 recovery logs and incomplete phases."""
    markers = [
        'large upload cancellation adapter: Metal ',
        'raster replacement values: format=Rgba8UnormSrgb; initial_center=[255, 0, 0, 255]; retry_center=[0, 0, 255, 255]',
        'raster replacement values: format=Rgba16Float; initial_center=[17408, 47104, 0, 15360]; retry_center=[46080, 0, 17408, 15360]',
    ]
    markers += [f'raster replacement recovery: format={fmt}; pressure/cancellation previous frame exact; retry blue exact; final GPU/queue credit=0'
                for fmt in ('Rgba8UnormSrgb', 'Rgba16Float')]
    phases = 'raster upload cancellation: total_checkpoints=2933; exercised=[1, 2, 1466, 2931, 2932, 2933]; upload_budget=192000000'
    pressure = 'raster upload pressure: texture-and-queue-refused; previous-frame-exact; temporary-credit-zero'
    return (passed == 1 and ignored == 0 and all(text.count(marker) == 1 for marker in markers)
            and text.count(phases) == 2 and text.count(pressure) == 2)


def raw_worker_navigation_evidence(text, passed, ignored):
    directions = re.findall(r'RAW worker navigation: direction-index=(\d+); 6188x4120 frame and original samples exact; disk-count=1; final=0', text)
    hdr = re.findall(r'RAW worker HDR: (\d+x\d+); all scene-linear binary16 components exact and finite', text)
    return (passed == 1 and ignored == 0
            and 'RAW worker navigation adapter: Metal ' in text
            and 'RAW worker navigation: 1920x1080 Metal frame exact' in text
            and 'RAW worker navigation: 6188x4120 Metal frame exact' in text
            and re.search(r'reverse prefetch: 25494560 samples exact; cancelled forward absent; one backward entry; peak=\d+ final=0', text) is not None
            and sorted(directions) == ['0', '2']
            and sorted(hdr) == ['1920x1080', '6188x4120', '6188x4120', '6188x4120'])


def large_lod_evidence(text, passed, ignored):
    markers = ['36 MP HDR LOD adapter: Metal ',
               '36 MP HDR area LOD: source=576000000; output=16000000; all1000000 pixels exact; SDR/HDR Metal exact;',
               'output root=0']
    markers += [f'36 MP HDR area LOD full output: {size}; every SDR/HDR component matches independent constant reference'
                for size in ('64x64', '1000x1000')]
    return passed == 1 and ignored == 0 and all(text.count(marker) == 1 for marker in markers)


def raster_timing_evidence(text, passed, ignored):
    lines = [line for line in text.splitlines() if line.startswith('{"median_gpu_ms":')]
    if passed != 1 or ignored != 0 or len(lines) != 1 or 'backend: Metal' not in text:
        return False
    try:
        row = json.loads(lines[0])
        samples = sorted(row['samples_gpu_ms'])
        return (len(samples) == 30 and row['warmups'] == 5
                and row['source'] == [4096, 4096] and row['target'] == [2048, 2048]
                and row['target_format'] == 'Rgba16Float'
                and all(math.isfinite(value) and value > 0 for value in samples)
                and math.isclose(row['median_gpu_ms'], (samples[14] + samples[15]) / 2, rel_tol=1e-12)
                and row['p95_gpu_ms'] == samples[28] and row['max_gpu_ms'] == samples[29])
    except (KeyError, TypeError, ValueError, IndexError):
        return False


def hdr_range_evidence(text, passed, ignored):
    markers = ['HDR finite-range adapter: Metal ', 'HDR opposite-extreme adapter: Metal ',
               'HDR opposite-extreme: 63x65; symmetric center exact; all components finite']
    markers += [f'HDR finite-range: exposure={stops}; all4096 pixels exact; finite signed binary16'
                for stops in (-10, 0, 10)]
    return passed == 2 and ignored == 0 and all(text.count(marker) == 1 for marker in markers)


def selected_page_cancel_evidence(text, passed, ignored):
    marker = 'Selected-page admitted cancellation: bytes=67108864; obsolete page1 unpublished; RAM=0; page2 retry payload/SDR/HDR exact; blocked decoder bypassed; final=0'
    return (passed == 1 and ignored == 0
            and text.count('Selected-page admitted cancellation adapter: Metal ') == 1
            and text.count(marker) == 1)


def ai_swap_evidence(text, passed, ignored):
    cases = re.findall(r'^AI swap plane: (.+); kind=(srgb8|linear32); samples/frame exact; pressure/cancel retry; queue=0$', text, re.MULTILINE)
    expected = {(name, kind) for name in ['VectorApple.ai-page-0.native.rgba',
                'one.ai-page-0.native.rgba', 'one.ai-page-1.native.rgba']
                for kind in ('srgb8', 'linear32')}
    return (passed == 1 and ignored == 0 and len(cases) == 6 and set(cases) == expected
            and 'AI swap Metal adapter: Metal ' in text
            and 'AI Metal swap: pages=3; storage_cases=6; root=0; production_AI_fidelity_unqualified' in text)


def ai_transport_evidence(text, passed, ignored):
    cases = re.findall(r'^AI Metal plane: (.+); pixels=(\d+); max_RGB8_error=(\d+); final_credit=0$', text, re.MULTILINE)
    expected = {'VectorApple.ai-page-0.native.rgba': 74046,
                'one.ai-page-0.native.rgba': 1049088,
                'one.ai-page-1.native.rgba': 30975}
    return (passed == 1 and ignored == 0 and len(cases) == 3
            and {name: int(pixels) for name, pixels, _ in cases} == expected
            and all(int(error) <= 1 for _, _, error in cases)
            and 'AI transport Metal adapter: Metal ' in text
            and 'AI Metal transport: pages=3; checked_pixels=1184109; production_AI_fidelity_unqualified' in text)


def compute_cache_evidence(text, passed, ignored):
    """Require both full HDR transport cases, including RAM pinning and disk restore."""
    hdr = re.findall(
        r'compute cache HDR: (\d+x\d+); finite signed HDR; computed-pinned-restored binary16 exact', text)
    pipeline = re.findall(
        r'compute cache pipeline: (\d+x\d+); all component bits and (\d+x\d+) Metal frames exact; root=0; gpu=0; one swap write/read', text)
    recovery = re.findall(r'compute cache recovery: (\d+x\d+); full-budget refusal; admitted cancellation; exact SDR-HDR retry; final=0', text)
    return (passed == 2 and ignored == 0
            and len(recovery) == 2 and set(recovery) == {'164x50', '4000x3000'}
            and len(hdr) == 2 and set(hdr) == {'164x50', '4000x3000'}
            and len(pipeline) == 2
            and set(pipeline) == {('164x50', '128x96'), ('4000x3000', '4000x3000')}
            and text.count('compute cache adapter: Metal ') == 2)


def raw_collection_evidence(text, seconds, file_count):
    """Require final workload evidence in addition to a passing test harness."""
    matches = re.findall(
        r'RAW collection complete: rounds=(\d+); elapsed=([\d.]+)s; reads=(\d+); writes=(\d+); root=0; queue=0', text)
    if len(matches) != 1:
        return False
    rounds, elapsed, reads, writes = matches[0]
    residency = re.findall(
        r'RAW persistent GPU complete: resident_peak=(\d+); upload_peak=(\d+); queue_peak=(\d+); resident=0; upload=0; queue=0', text)
    pressure = re.findall(
        r'RAW GPU pressure complete: refusals=(\d+); previous-frame-exact; reservation-unchanged; retry-exact', text)
    cancellations = re.findall(
        r'RAW partial GPU cancellation complete: cancellations=(\d+); checks=64; tile=512; halo=2; previous-frame-exact; scratch-queue-released; retry-exact', text)
    hdr = re.findall(r'RAW collection HDR complete: frames=(\d+); binary16-exact; finite; persistent-renderer; SDR-HDR-shared-root', text)
    hdr_sources = re.findall(r'^RAW collection HDR seed: source=(.+); binary16=finite; above_one=\d+; negative=\d+$', text, re.MULTILINE)
    hdr_failures = re.findall(
        r'RAW HDR replacement failures complete: pressure=(\d+); cancellations=(\d+); binary16-previous-exact; credit-retained; temporary-credit-zero; retry-exact', text)
    return (int(rounds) >= 10 and float(elapsed) >= seconds
            and int(reads) == int(rounds) * file_count and int(writes) == file_count
            and len(residency) == 1 and all(int(value) > 0 for value in residency[0])
            and len(pressure) == 1 and int(pressure[0]) == int(reads) - 1
            and len(cancellations) == 1 and int(cancellations[0]) == int(reads) - 1
            and len(hdr) == 1 and int(hdr[0]) == int(reads)
            and len(hdr_sources) == file_count and len(set(hdr_sources)) == file_count
            and len(hdr_failures) == 1 and all(int(count) == int(reads) - 1 for count in hdr_failures[0]))

def raster_collection_evidence(text):
    cases = re.findall(r'^raster collection: (.+); index=(\d+); prepared=(false|true); bits/color/frame exact; root=0; queue=0$',
                       text, re.MULTILINE)
    hdr_cases = re.findall(r'^raster collection HDR: (.+); index=(\d+); prepared=(false|true); binary16 frame exact$',
                           text, re.MULTILINE)
    sources = {source for source, _, _ in cases}
    cicp_sources = {f'raster/cicp-{name}.png' for name in
                    ('linear-2020', 'linear-p3', 'pq-2020', 'hlg-2020', 'narrow-709', 'encoded-p3')}
    cicp_sources.update(('raster/gamma-chrm-709.png', 'raster/gamma-chrm-p3.png', 'raster/gamma-gray.png', 'raster/gamma-gray-packed-1.png', 'raster/gamma-gray-packed-2.png', 'raster/gamma-gray-packed-4.png'))
    selections = {(source, index) for source, index, _ in cases}
    bounds = re.findall(r'^raster collection bounds: (.+); rejected_index=(\d+); retained_bytes_unchanged$',
                        text, re.MULTILINE)
    counts = dict(bounds)
    return (cicp_sources <= sources
            and len(sources) == 118 and len(selections) == 147 and len(cases) == 294 and len(set(cases)) == 294
            and len(hdr_cases) == 294 and set(hdr_cases) == set(cases)
            and len(bounds) == len(counts) == 118 and set(counts) == sources
            and all({int(index) for name, index in selections if name == source} == set(range(int(count)))
                    for source, count in bounds)
            and all((source, index, mode) in cases for source, index in selections for mode in ('false', 'true'))
            and all((source, '0', mode) in cases for source in sources for mode in ('false', 'true'))
            and 'raster collection HDR oracle: 63x65; 8,-2,1,1; every binary16 pixel exact' in text
            and 'raster collection complete: sources=118; selections=147; storage_cases=294; root=0; queue=0' in text)


def metal_extent_evidence(text):
    extents = re.findall(r'viewer Metal full extent: portable=(\d+); supported=(\d+); width=(\d+); height=128; every_SDR_HDR_pixel_valid; coordinate_pattern=true; portable_crop_cases=4; root=0; texture=0; portable_texture=0', text)
    return (len(extents) == 2 and len({(p, s) for p, s, _ in extents}) == 1
            and {int(w) for _, _, w in extents} == {int(extents[0][0]) + 1, int(extents[0][1])})


def linear_color_evidence(text):
    required = [
        'declared_exr_wide_gamuts_preserve_native_color_and_metal_swap',
        'radiance_arbitrary_primaries_and_non_d65_white_survive_swap_and_metal',
        'exr_arbitrary_primaries_and_aces_ids_preserve_color_through_metal_swap',
        'adopted_neutral_metadata_survives_v4_swap_pressure_and_metal',
    ]
    markers = [
        'EXR wide color: 10 qualified metadata cases; 4 conflicting cases refused; 2 unknown-ID cases remain unqualified; root=0',
        'Radiance arbitrary color: 3 non-D65 RGBW sources; independent full matrix oracle; native samples unchanged; Metal SDR/HDR and swap retry exact',
        'EXR arbitrary color: accepted=17; refused=11; unknown=6; independent full matrix; native/swap/Metal SDR/HDR exact; root=0',
        'Adopted neutral metadata: cases=2; independent full matrix; V4 pressure/cancel/retry; Metal SDR/HDR exact; root=0',
    ]
    return (all(re.search(r'^test ' + re.escape(test) + r' \.\.\. ok$', text, re.MULTILINE)
                for test in required)
            and all(marker in text for marker in markers)
            and 'Declared linear gamut adapter: Metal ' in text)


RAW_NUMERIC_TESTS = (
    'signed_f32_extremes_interpolate_finitely_against_f64_reference',
    'finite_large_bayer_values_refuse_unsafe_filter_range_before_replacement',
    'nonfinite_raw_view_components_keep_previous_finite_frame',
    'signed_rgbe_hdr_saturation_preserves_float32_and_both_half_limits',
    'nonfinite_raster_controls_produce_exact_finite_sdr_hdr_frames',
)


def raw_numeric_evidence(text):
    return all(re.search(r'^test ' + re.escape(name) + r' \.\.\. ok$', text, re.MULTILINE)
               for name in RAW_NUMERIC_TESTS)



CACHE_TTL_TESTS = (
    'swap::raster_tests::idle_ttl_maintenance_releases_files_and_quota_but_keeps_live_handles',
    'swap::raster_tests::ttl_worker_reclaims_idle_swap_without_foreground_calls',
    'swap::raster_tests::ttl_worker_reclaims_swap_under_continuous_barriers',
    'swap::raster_tests::nonzero_ttl_keeps_fresh_swap_then_reclaims_without_lookup',
)


def cache_ttl_evidence(text):
    return all(re.search(r'^test ' + re.escape(name) + r' \.\.\. ok$', text, re.MULTILINE)
               for name in CACHE_TTL_TESTS)


def large_float_storage_evidence(text, minimum_seconds):
    matches = re.findall(r'large float delayed swap: cycles=(\d+); seconds=([\d.]+);2048x2048;64MiB; every signed/HDR/negative-zero bit exact; full pressure and admitted cancellation after64KiB read each cycle; disk count2/bytes bounded; final=0', text)
    return (len(matches) == 1 and int(matches[0][0]) >= 3
            and float(matches[0][1]) >= minimum_seconds)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target-dir', type=Path, required=True)
    parser.add_argument('--output-dir', type=Path, required=True,
                        help='New directory; existing results are never overwritten')
    parser.add_argument('--gpu-backend', default='metal' if sys.platform == 'darwin' else 'auto',
                        choices=['auto', 'metal', 'vulkan', 'dx12', 'gl'],
                        help='Required backend for hardware suites; unavailable devices fail')
    parser.add_argument('--storage-soak-seconds', type=int, default=0,
                        help='Run three small delayed-I/O modes plus 64MiB float mode for this duration each (float mode always runs at least three cycles), 100000-key churn, idle TTL and cancellation/queue controls; zero disables')
    parser.add_argument('--metal-policy-cycles', type=int, default=0,
                        help='Run small GPU policy cycles plus one 12 MP HDR policy/swap cycle; zero disables')
    parser.add_argument('--metal-large-policy-cycles', type=int, default=0,
                        help='Repeat changing 12 MP payloads through policy updates, pressure refusal, swap and full Metal readback; zero disables')
    parser.add_argument('--metal-collection-seconds', type=int, default=0,
                        help='Run eight 12 MP frames through restores, pressure and TTL; full SDR and 63x65 HDR readback by default; zero disables')
    parser.add_argument('--metal-collection-full-hdr', action='store_true',
                        help='Use 4000x3000 HDR readback; requires --metal-collection-seconds')
    parser.add_argument('--metal-raw-collection-manifest', type=Path,
                        help='Required RAW file paths, one per line; run real-camera collection restore/render checks')
    parser.add_argument('--metal-raw-collection-seconds', type=int, default=0,
                        help='Minimum real RAW collection duration; zero keeps ten rounds; requires manifest')
    parser.add_argument('--metal-mixed-raw-workers', action='store_true',
                        help='Run actual mixed-camera and three-neighbor RAW workers; requires a manifest with at least five files')
    parser.add_argument('--metal-raster-collection', action='store_true',
                        help='Check 118 raster sources at all 147 selections with bounds, native/prepared RAM, pressure, cancellation, swap and GPU readback')
    parser.add_argument('--metal-prefetch-navigation', action='store_true',
                        help='Compare Metal frames during mixed-format viewer preload reversals and memory pressure')
    parser.add_argument('--metal-linear-import', action='store_true',
                        help='Require all five PFM/ASTC/KTX2/DPX linear import and PFM row-order Metal swap checks')
    parser.add_argument('--metal-raw-worker-navigation', action='store_true',
                        help='Run actual RAW prefetch worker reversal with EOS R8 fixtures and full-sensor Metal readback')
    parser.add_argument('--metal-large-lod', action='store_true',
                        help='Check all36MP HDR area-downsample samples and full1000-square Metal SDR/HDR output')
    parser.add_argument('--metal-raster-timing', action='store_true',
                        help='Measure actual Metal render-pass timestamps; excludes decode, upload and presentation')
    parser.add_argument('--metal-hdr-range', action='store_true',
                        help='Require empirical Metal signed binary16 exposure and opposite-f32-extreme interpolation checks; not portable qualification')
    parser.add_argument('--metal-large-float-hdr', action='store_true',
                        help='Check full 12 MP RGBA16Float attachment with independent half-float oracle and disk swap')
    parser.add_argument('--metal-viewer-ttl-prefetch', action='store_true',
                        help='Check viewer EXR/HDR and APNG TTL restores and preloads bypassing a blocked decoder with Metal readback')
    parser.add_argument('--metal-large-upload-cancel', action='store_true',
                        help='Check 12 MP upload cancellation at validation/admission boundaries and recovery on actual GPU')
    parser.add_argument('--metal-selected-page-cancel', action='store_true',
                        help='Require generation cancellation after64MiB restore admission and new-page blocked-decoder Metal SDR/HDR retry')
    parser.add_argument('--metal-png-physical-aspect', action='store_true',
                        help='Check PNG and selected APNG pixel aspect through color preparation, swap and Metal SDR/HDR')
    parser.add_argument('--metal-ico-pages', action='store_true',
                        help='Check selected ICO/CUR quality order, alpha/hotspots, RAM/count/TTL, swap and actual Metal')
    parser.add_argument('--metal-tiff-pages', action='store_true',
                        help='Check selected classic/BigTIFF and independent Deflate16/float pages through native precision, metadata, RAM/TTL, swap and Metal')
    parser.add_argument('--metal-large-apng', action='store_true',
                        help='Check12MP16-bit APNG memory pressure retry, exact samples, swap and Metal SDR/HDR')
    parser.add_argument('--metal-compute-cache', action='store_true',
                        help='Check compute output through RAM, disk swap and Metal, including a full 12 MP frame')
    parser.add_argument('--metal-model-upload-worker', action='store_true',
                        help='Qualify bounded model GPU worker burst and completed-result cancellation')
    parser.add_argument('--metal-model-swap-cycles', type=int, default=0,
                        help='Repeat model swap restores with pressure/cancellation and Metal readback; zero disables')
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
    parser.add_argument('--mef-sensor-source', type=Path,
                        help='Run full original Mamiya ZD sensor transport on Metal; does not qualify camera color')
    parser.add_argument('--ai-native-root', type=Path,
                        help='Run pinned native AI replay-plane Metal transport; production AI fidelity remains unqualified')
    args = parser.parse_args()
    if args.storage_soak_seconds and args.storage_soak_seconds < 10:
        parser.error('--storage-soak-seconds must be zero or at least 10')
    if not 0 <= args.metal_policy_cycles <= 10000:
        parser.error('--metal-policy-cycles must be between zero and 10000')
    if not 0 <= args.metal_large_policy_cycles <= 1000:
        parser.error('--metal-large-policy-cycles must be between zero and 1000')
    if args.metal_collection_seconds and not 10 <= args.metal_collection_seconds <= 86400:
        parser.error('--metal-collection-seconds must be zero or between 10 and 86400')
    if args.metal_collection_full_hdr and not args.metal_collection_seconds:
        parser.error('--metal-collection-full-hdr requires --metal-collection-seconds')
    if not 0 <= args.metal_model_swap_cycles <= 1000:
        parser.error('--metal-model-swap-cycles must be between zero and 1000')
    if not 0 <= args.metal_raw_collection_seconds <= 86400:
        parser.error('--metal-raw-collection-seconds must be between zero and 86400')
    if args.metal_raw_collection_seconds and not args.metal_raw_collection_manifest:
        parser.error('--metal-raw-collection-seconds requires --metal-raw-collection-manifest')
    if args.metal_mixed_raw_workers and not args.metal_raw_collection_manifest:
        parser.error('--metal-mixed-raw-workers requires --metal-raw-collection-manifest')
    if (args.metal_selected_page_cancel or args.metal_large_upload_cancel) and args.gpu_backend != 'metal':
        parser.error('signed HDR/selected-page recovery qualification requires --gpu-backend metal')
    if (args.metal_hdr_range or args.metal_large_lod or args.metal_raster_timing) and args.gpu_backend != 'metal':
        parser.error('Metal HDR range/LOD/timing modes require --gpu-backend metal')
    environment = os.environ.copy()
    environment['RRRAH_GPU_OPTIONAL'] = '0'
    environment['RRRAH_GPU_BACKEND'] = args.gpu_backend
    environment['RRRAH_STRESS_SECONDS'] = str(args.storage_soak_seconds)
    environment['RRRAH_METAL_MODEL_SWAP_CYCLES'] = str(args.metal_model_swap_cycles or 20)
    environment['RRRAH_METAL_COLLECTION_FULL_HDR'] = '1' if args.metal_collection_full_hdr else '0'
    environment['RRRAH_METAL_COLLECTION_SECONDS'] = str(args.metal_collection_seconds or 60)
    environment['RRRAH_METAL_LARGE_POLICY_CYCLES'] = str(args.metal_large_policy_cycles or 30)
    environment['RRRAH_METAL_POLICY_CYCLES'] = str(args.metal_policy_cycles or 1)
    external_inputs = {}
    if args.ai_native_root:
        if args.gpu_backend != 'metal':
            parser.error('--ai-native-root requires --gpu-backend metal')
        ai_root = args.ai_native_root.resolve()
        pins = json.loads((ROOT / 'tests/fixtures/ai-metal-replay-planes.json').read_text())['files']
        for name, expected in pins.items():
            path = ai_root / name
            if not path.is_file() or hashlib.sha256(path.read_bytes()).hexdigest() != expected:
                parser.error(f'--ai-native-root missing or mismatched pinned input: {name}')
            external_inputs[f'ai-native/{name}'] = {'path': str(path), 'sha256': expected}
        environment['RRRAH_AI_METAL_ROOT'] = str(ai_root)
        environment['RRRAH_AI_CORPUS'] = str(ai_root / 'ai-corpus')
    if args.mef_sensor_source:
        if args.gpu_backend != 'metal':
            parser.error('--mef-sensor-source requires --gpu-backend metal')
        source = args.mef_sensor_source.resolve()
        expected = json.loads((ROOT / 'tests/fixtures/mef-investigation.json').read_text())['source_sha256']
        if not source.is_file() or hashlib.sha256(source.read_bytes()).hexdigest() != expected:
            parser.error('--mef-sensor-source requires the pinned original Mamiya ZD MEF')
        environment['RRRAH_MEF_SOURCE'] = str(source)
        external_inputs['Mamiya-ZD.MEF'] = {'path': str(source), 'sha256': expected}
    if args.metal_raw_collection_manifest:
        manifest = args.metal_raw_collection_manifest.resolve()
        if not manifest.is_file():
            parser.error('RAW collection manifest must exist')
        paths = [Path(line) for line in manifest.read_text().splitlines() if line.strip()]
        if len(paths) < 2 or len(set(paths)) != len(paths) or not all(path.is_absolute() and path.is_file() for path in paths):
            parser.error('RAW collection requires at least two distinct existing absolute file paths')
        if args.metal_mixed_raw_workers and len(paths) < 5:
            parser.error('--metal-mixed-raw-workers requires at least five RAW files')
        environment['RRRAH_RAW_COLLECTION_MANIFEST'] = str(manifest)
        environment['RRRAH_RAW_COLLECTION_SECONDS'] = str(args.metal_raw_collection_seconds)
        external_inputs['raw_collection'] = [{'path': str(path), 'sha256': hashlib.sha256(path.read_bytes()).hexdigest()} for path in paths]
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
        ('hdr-ttl-gpu', ['-p', 'rrrah', '--test', 'hdr_raster_swap', '--test', 'ttl_swap_metal'], ['--', '--nocapture']),
        ('camera-color-gpu', ['-p', 'rrrah-gpu', '--test', 'camera_color_readback'], ['--', '--nocapture']),
        ('gpu', ['-p', 'rrrah-gpu', '--lib', '--test', 'compute_readback', '--test', 'readback', '--test', 'raster_readback', '--test', 'model_readback', '--test', 'raw_linear_readback', '--test', 'rgbe_compute_readback', '--test', 'rgbe_view_readback'], ['--', '--nocapture']),
    ]
    if args.gpu_only:
        # Keep named harness results intact; parallel uncaptured stderr can
        # split a passing result line and invalidate mandatory-test evidence.
        suites = [('gpu-all', ['-p', 'rrrah-gpu'], ['--', '--show-output'])]
    if args.gpu_backend == 'metal' and not args.gpu_only:
        # Ordinary viewer tests intentionally skip these hardware cases. Keep
        # source residency and navigation mandatory in the default Metal gate.
        suites.extend([
            ('gpu-swapped-viewport', ['-p', 'rrrah', '--bin', 'rrrah', 'raster_viewport::tests'],
             ['--', '--ignored', '--nocapture', '--test-threads=1']),
            ('gpu-swapped-source-navigation', ['-p', 'rrrah', '--test', 'swapped_region_metal'],
             ['--', '--include-ignored', '--nocapture', '--test-threads=1']),
        ])
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
    if args.metal_large_policy_cycles:
        suites.append(('gpu-policy-large-sustained', ['-p', 'rrrah', '--test', 'ttl_swap_metal',
                        'sustained_large_hdr_live_policy_preserves_every_component_and_metal_pixel'],
                       ['--', '--exact', '--ignored', '--nocapture']))
    if args.metal_collection_seconds:
        suites.append(('gpu-collection-long-lived', ['-p', 'rrrah', '--test', 'ttl_swap_metal',
                        'long_lived_large_collection_cache_preserves_swap_and_metal_frames'],
                       ['--', '--exact', '--ignored', '--nocapture']))
    if args.metal_raw_collection_manifest:
        suites.append(('gpu-real-raw-collection', ['-p', 'rrrah', '--test', 'raw_collection_metal',
                        'real_raw_collection_eviction_and_swap_preserve_metal_frames'],
                       ['--', '--exact', '--ignored', '--nocapture']))
    if args.metal_mixed_raw_workers:
        for name, test in [
                ('gpu-mixed-raw-worker', 'mixed_camera_worker_prefetch_preserves_full_metal_frames'),
                ('gpu-multi-neighbor-raw-worker', 'multi_neighbor_raw_worker_preserves_metal_frames')]:
            suites.append((name, ['-p', 'rrrah', '--bin', 'rrrah', 'gallery::tests::' + test],
                           ['--', '--exact', '--ignored', '--nocapture', '--test-threads=1']))
    if args.metal_raster_collection:
        suites.append(('gpu-raster-collection', ['-p', 'rrrah', '--test', 'raster_collection_metal',
                        'raster_collection_native_and_prepared_swap_preserve_metal_frames'],
                       ['--', '--exact', '--ignored', '--nocapture', '--test-threads=1']))
    if args.metal_prefetch_navigation:
        suites.append(('gpu-prefetch-navigation', ['-p', 'rrrah', '--bin', 'rrrah',
                        'mixed_gallery_reversals_preserve_metal_frames_and_prefetch_pressure'],
                       ['--', '--ignored', '--nocapture', '--test-threads=1']))
    if args.metal_linear_import:
        for test in (
                'linear_cli_pfm_preload_swap_preserves_metal_sdr_and_hdr',
                'linear_cli_astc_hdr_preload_swap_preserves_metal_sdr_and_hdr',
                'linear_cli_ktx2_hdr_preload_swap_preserves_metal_sdr_and_hdr',
                'linear_cli_dpx_preload_swap_preserves_metal_sdr_and_hdr',
                'pfm_top_down_cli_swap_preserves_metal_sdr_hdr_rows'):
            suites.append(('gpu-linear-import-' + test, ['-p', 'rrrah', '--bin', 'rrrah',
                           'linear_pfm_viewer_tests::' + test],
                           ['--', '--exact', '--ignored', '--nocapture', '--test-threads=1']))
    if args.metal_raw_worker_navigation:
        suites.append(('gpu-raw-worker-navigation', ['-p', 'rrrah', '--bin', 'rrrah',
                        'navigation_reversal_worker_preserves_full_raw_metal_frames'],
                       ['--', '--ignored', '--nocapture', '--test-threads=1']))
    if args.metal_large_lod:
        suites.append(('gpu-large-lod', ['-p', 'rrrah-gpu', '--test', 'raster_readback',
                        'large_hdr_area_lod_preserves_all_output_samples_and_metal_frame'],
                       ['--', '--exact', '--ignored', '--nocapture']))
    if args.metal_raster_timing:
        suites.append(('gpu-raster-timing', ['-p', 'rrrah-gpu', '--test', 'raster_timing'],
                       ['--', '--ignored', '--nocapture']))
    if args.metal_hdr_range:
        suites.append(('gpu-hdr-range', ['-p', 'rrrah-gpu', '--test', 'raster_readback', 'hdr_'],
                       ['--', '--ignored', '--nocapture', '--test-threads=1', '--skip', 'large_hdr_area_lod']))
    if args.metal_large_float_hdr:
        suites.append(('gpu-large-float-hdr', ['-p', 'rrrah', '--test', 'hdr_raster_swap',
                        'large_float_hdr_swap_preserves_full_metal_attachment'],
                       ['--', '--exact', '--ignored', '--nocapture']))
    if args.metal_viewer_ttl_prefetch:
        suites.append(('gpu-viewer-ttl-prefetch', ['-p', 'rrrah', '--bin', 'rrrah',
                        'repeated_hdr_ttl_viewer_prefetch_restore_preserves_metal_frames'],
                       ['--', '--ignored', '--nocapture', '--test-threads=1']))
        suites.append(('gpu-apng-viewer-ttl-prefetch', ['-p', 'rrrah', '--bin', 'rrrah',
                        'apng_selected_frame_ttl_prefetch_preserves_metal_sdr_hdr'],
                       ['--', '--ignored', '--show-output', '--test-threads=1']))
    if args.metal_large_upload_cancel:
        suites.append(('gpu-large-upload-cancel', ['-p', 'rrrah-gpu', '--test', 'raster_readback',
                        'large_cancelled_upload_validation_and_admission_preserve_visible_texture'],
                       ['--', '--exact', '--ignored', '--nocapture']))
    if args.metal_selected_page_cancel:
        suites.append(('gpu-selected-page-cancel', ['-p', 'rrrah', '--bin', 'rrrah',
                        'raster_swap_viewer_tests::selected_page_prefetch_cancel_after_restore_admission_metal'],
                       ['--', '--exact', '--ignored', '--nocapture']))
    if args.metal_png_physical_aspect:
        suites.append(('gpu-png-physical-aspect', ['-p', 'rrrah', '--test', 'png_physical_aspect'],
                       ['--', '--ignored', '--show-output']))
    if args.metal_ico_pages:
        suites.append(('gpu-ico-pages', ['-p', 'rrrah', '--test', 'ico_pages_metal'],
                       ['--', '--ignored', '--show-output']))
        suites.append(('gpu-ico-viewer-pages', ['-p', 'rrrah', '--bin', 'rrrah',
                        'raster_swap_viewer_tests::ico_cur_selected_images_loader_cache_and_blocked_prefetch_metal'],
                       ['--', '--exact', '--ignored', '--show-output']))
    if args.metal_tiff_pages:
        suites.append(('gpu-tiff-pages', ['-p', 'rrrah', '--test', 'tiff_pages_metal'],
                       ['--', '--ignored', '--show-output']))
        suites.append(('gpu-tiff-viewer-pages', ['-p', 'rrrah', '--bin', 'rrrah',
                        'raster_swap_viewer_tests::tiff_selected_pages_loader_cache_and_blocked_prefetch_metal'],
                       ['--', '--exact', '--ignored', '--show-output']))
    if args.metal_large_apng:
        suites.append(('gpu-large-apng', ['-p', 'rrrah', '--test', 'large_apng_memory'],
                       ['--', '--ignored', '--show-output']))
    if args.metal_compute_cache:
        suites.append(('gpu-compute-cache', ['-p', 'rrrah', '--test', 'compute_cache_swap'],
                       ['--', '--ignored', '--nocapture', '--test-threads=1']))
    if args.metal_model_upload_worker:
        suites.append(('gpu-model-upload-worker', ['-p', 'rrrah', '--bin', 'rrrah',
                        'model_upload_worker::'],
                       ['--', '--ignored', '--nocapture', '--test-threads=1']))
    if args.metal_model_swap_cycles:
        suites.append(('gpu-model-swap-cycles', ['-p', 'rrrah', '--test', 'model_readback',
                        'repeated_model_swap_pressure_and_cancellation_preserve_metal_frames'],
                       ['--', '--exact', '--ignored', '--nocapture']))
    if args.full_sensor_compute:
        suites.append(('full-sensor-compute', ['-p', 'rrrah-gpu', '--test', 'compute_readback', 'full_sensor'],
                       ['--', '--include-ignored', '--nocapture', '--test-threads=1']))
    if args.mef_sensor_source:
        suites.append(('mef-full-sensor-metal', ['-p', 'rrrah', '--test', 'mef_sensor_compute'],
                       ['--', '--ignored', '--nocapture', '--test-threads=1']))
    if args.external_viewer_corpus:
        suites.append(('external-prefetch', ['-p', 'rrrah', '--bin', 'rrrah', 'prefetch'],
                       ['--', '--include-ignored', '--nocapture', '--test-threads=1']))
        suites.append(('external-eip-ttl', ['-p', 'rrrah', '--test', 'ttl_swap_metal',
                        'expired_eip_lease_and_swap_restore_match_original_cr3_metal_frame'],
                       ['--', '--include-ignored', '--nocapture']))
    if args.ai_native_root:
        suites.append(('ai-native-metal-transport', ['-p', 'rrrah-gpu', '--test', 'ai_native_transport'],
                       ['--', '--ignored', '--nocapture']))
        if not args.gpu_only:
            suites.append(('ai-ram-swap-metal', ['-p', 'rrrah', '--test', 'ai_swap_metal'],
                           ['--', '--ignored', '--nocapture']))
    metal_cases = {}
    if args.gpu_backend == 'metal' and not args.gpu_only:
        metal_cases.update({
            'metal-thumbnail-completion': ('thumbnail_budget_callbacks_progress_without_surface_redraw',
                ['thumbnail upload completion: rounds=2; pressure_refused=2; metadata_blocked=true; root=0; queue=0']),
            'metal-supported-texture-extent': ('viewer_uses_supported_metal_texture_extent_above_default',
                ['width=8193; height=128; every_SDR_HDR_pixel_valid; coordinate_pattern=true; portable_crop_cases=4; root=0; texture=0; portable_texture=0',
                 'every_SDR_HDR_pixel_valid; coordinate_pattern=true; portable_crop_cases=4; root=0; texture=0; portable_texture=0']),
        })
        if args.ai_native_root:
            metal_cases.update({
                'metal-ai-production-cache': ('real_ai_cached_pages_preserve_metal_sdr_hdr_under_full_pressure',
                    ['real AI cache: pages=3; independent_keys=3; root=0; appearance_unqualified',
                     'real AI cache adapter: Metal ']),
                'metal-ai-queued-reversal': ('queued_ai_page_reversal_cancels_stale_preload_and_preserves_metal',
                    ['AI queued navigation: page1->page0->page1; stale preload refused; pending_latest_only; independent_keys=2; full_pressure_shared_hit; Metal_SDR_HDR_exact; root=0; appearance_unqualified']),
                'metal-ai-render-cancel': ('real_ai_cancel_after_render_scratch_admission_recovers_metal',
                    ['typed_cancelled; retry_samples_Metal_SDR_HDR_exact; root=0; reference_root=0; appearance_unqualified',
                     'AI render cancellation adapter: Metal ']),
            })
        for name, (test, _) in metal_cases.items():
            suites.append((name, ['-p', 'rrrah', '--bin', 'rrrah', test],
                           ['--', '--ignored', '--nocapture', '--test-threads=1']))
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
               'log': str(log),
               'log_sha256': hashlib.sha256(log.read_bytes()).hexdigest()}
        row['verified_execution'] = (process.returncode == 0 and row['passed'] > 0
                                     and row['failed'] == 0)
        if name in ('gpu-large-upload-cancel', 'gpu-selected-page-cancel'):
            verifier = (signed_upload_recovery_evidence if name == 'gpu-large-upload-cancel'
                        else selected_page_cancel_evidence)
            row['workload_evidence_valid'] = verifier(text, row['passed'], row['ignored'])
            row['verified_execution'] &= row['workload_evidence_valid']
        if name in ('gpu-large-lod', 'gpu-raster-timing'):
            verifier = large_lod_evidence if name == 'gpu-large-lod' else raster_timing_evidence
            row['workload_evidence_valid'] = verifier(text, row['passed'], row['ignored'])
            row['verified_execution'] &= row['workload_evidence_valid']
        if name == 'gpu-hdr-range':
            row['workload_evidence_valid'] = hdr_range_evidence(text, row['passed'], row['ignored'])
            row['verified_execution'] &= row['workload_evidence_valid']
        if name == 'gpu-collection-long-lived':
            expected_output = [4000, 3000] if args.metal_collection_full_hdr else [63, 65]
            row['hdr_output'] = expected_output
            row['workload_evidence_valid'] = collection_evidence(
                text, args.metal_collection_seconds, args.metal_collection_full_hdr,
                row['passed'], row['ignored'])
            row['verified_execution'] &= row['workload_evidence_valid']
        if name == 'hdr-ttl-gpu' and args.gpu_backend == 'metal':
            row['workload_evidence_valid'] = linear_color_evidence(text)
            row['verified_execution'] &= row['workload_evidence_valid']
        if name in metal_cases:
            test, markers = metal_cases[name]
            row['required_test'] = test
            row['workload_evidence_valid'] = (row['passed'] == 1 and row['ignored'] == 0
                and all(marker in text for marker in markers))
            if name == 'metal-supported-texture-extent':
                row['workload_evidence_valid'] &= metal_extent_evidence(text)
            row['verified_execution'] = row['verified_execution'] and row['workload_evidence_valid']
        if name == 'cache':
            row['required_ttl_tests'] = list(CACHE_TTL_TESTS)
            row['ttl_evidence_valid'] = cache_ttl_evidence(text)
            row['verified_execution'] &= row['ttl_evidence_valid']
        if name == 'gpu-all':
            row['required_numeric_tests'] = list(RAW_NUMERIC_TESTS)
            row['numeric_evidence_valid'] = raw_numeric_evidence(text)
            row['verified_execution'] &= row['numeric_evidence_valid']
        if name == 'gpu-raw-worker-navigation':
            row['workload_evidence_valid'] = raw_worker_navigation_evidence(text, row['passed'], row['ignored'])
            row['verified_execution'] &= row['workload_evidence_valid']
        if name == 'ai-ram-swap-metal':
            row['workload_evidence_valid'] = ai_swap_evidence(text, row['passed'], row['ignored'])
            row['verified_execution'] = row['verified_execution'] and row['workload_evidence_valid']
        if name == 'ai-native-metal-transport':
            row['workload_evidence_valid'] = ai_transport_evidence(text, row['passed'], row['ignored'])
            row['verified_execution'] = row['verified_execution'] and row['workload_evidence_valid']
        if name == 'mef-full-sensor-metal':
            row['workload_evidence_valid'] = (
                row['passed'] == 1 and row['ignored'] == 0
                and 'backend: Metal' in text
                and 'MEF full sensor compute: samples=21461504; all authored channels exact;' in text
                and 'final=0; camera_WB_color_unqualified' in text)
            row['verified_execution'] = row['verified_execution'] and row['workload_evidence_valid']
        if name == 'storage-soak':
            required_tests = [
                'sustained_large_keyspace_delayed_swap_pressure_and_retry',
                'sustained_float_hdr_delayed_swap_pressure_cancel_and_retry',
                'sustained_byte_cap_evicts_without_count_or_ttl_pressure',
                'sustained_count_cap_evicts_without_byte_or_ttl_pressure',
                'hundred_thousand_distinct_rasters_keep_swap_index_and_memory_bounded',
                'idle_ttl_removes_delayed_large_blobs_and_physical_files',
                'cancelled_delayed_restore_releases_partial_pixels_and_keeps_retryable_entry',
                'stale_slow_restore_and_cancelled_lock_waiter_release_credit_for_latest_key',
                'burst_overload_of_delayed_writer_is_bounded_and_rejected_keys_retry',
            ]
            row['required_tests'] = required_tests
            row['workload_evidence_valid'] = (
                large_float_storage_evidence(text, args.storage_soak_seconds)
                and
                row['passed'] >= len(required_tests) and row['ignored'] == 0
                and all(f'test {test} ...' in text for test in required_tests)
                and 'writes=100000 reads=100000 pressure_refusals=1000' in text)
            row['verified_execution'] = row['verified_execution'] and row['workload_evidence_valid']
        if name in ('gpu-swapped-viewport', 'gpu-swapped-source-navigation'):
            markers = (
                ['36MP exact LOD swap cache:', 'exact_frame=true objects=2',
                 'swapped fit repeated pressure:8 downgrade/recovery cycles',
                 'corrupt selected swap block refused; previous HDR frame and preview retained']
                if name == 'gpu-swapped-viewport' else
                ['delayed swap navigation: old file/index released before latest wake; only new HDR source renders; final credits=0'])
            minimum = 7 if name == 'gpu-swapped-viewport' else 6
            row['workload_evidence_valid'] = (row['passed'] >= minimum and row['ignored'] == 0
                                              and all(marker in text for marker in markers))
            row['verified_execution'] = row['verified_execution'] and row['workload_evidence_valid']
        if name.startswith('gpu-linear-import-'):
            marker = ('PFM top-down rows: CLI, distinct cache/swap keys, decoder-free restore, SDR/HDR exact'
                      if name.endswith('pfm_top_down_cli_swap_preserves_metal_sdr_hdr_rows')
                      else 'CLI/preload/swap bits exact; Metal SDR/HDR exact; incompatible policies refused')
            row['workload_evidence_valid'] = (row['passed'] == 1 and row['ignored'] == 0
                                              and text.count(marker) == 1)
            row['verified_execution'] = row['verified_execution'] and row['workload_evidence_valid']
        if name == 'gpu-apng-viewer-ttl-prefetch':
            row['workload_evidence_valid'] = (
                row['passed'] == 1 and row['ignored'] == 0
                and text.count('APNG viewer TTL adapter: Metal ') == 1
                and text.count('APNG viewer TTL: four distinct frames; nonzero expiry; reverse prefetch under blocked decoder; SDR/HDR exact; final=0') == 1)
            row['verified_execution'] &= row['workload_evidence_valid']
        if name == 'gpu-viewer-ttl-prefetch':
            row['workload_evidence_valid'] = (
                row['passed'] == 1 and row['ignored'] == 0
                and text.count('viewer HDR TTL adapter: Metal ') == 1
                and text.count('viewer HDR TTL: 128 restores without decoder fallback plus 2 real preloads under blocked decoder; all Metal frames exact; final=0') == 1
                and text.count('viewer HDR TTL binary16: 130 finite HDR frames exact; above-one oracle retained') == 1)
            row['verified_execution'] = row['verified_execution'] and row['workload_evidence_valid']
        if name == 'gpu-model-upload-worker':
            row['workload_evidence_valid'] = (
                row['passed'] == 4 and row['ignored'] == 0
                and text.count('model stale submission: no thread/revision for obsolete initial job; current prepared geometry/revision/credit retained; final CPU/GPU zero') == 1
                and text.count('model first open cancellation: SDR/HDR generation/cancel/shutdown; six completed pipelines discarded; final CPU/GPU zero') == 1
                and text.count('model first open: worker-built SDR/HDR pipelines; owner-bound install; final CPU/GPU zero') == 1
                and text.count('model first open frames: 63x65 SDR/HDR; every sample equals synchronous path; exact geometry/depth budget') == 1
                and text.count('million grid restore failures: pressure=2; admitted-cancel=2; retry exact; temporary CPU zero') == 1
                and text.count('million grid swap complete: writes=1; reads=2; payload exact; final CPU/queue/GPU zero') == 1
                and text.count('million grid complete: triangles=1000000; 1920x1080 SDR/HDR exact; CPU/GPU credit zero') == 1
                and len(re.findall(r'million triangle render: output=16x16; every SDR pixel exact; render_readback_ms=[\d.]+; geometry=36000000; depth=1024', text)) == 1
                and len(re.findall(r'million triangle worker: decode_ms=[\d.]+; submit_ms=[\d.]+; ready_ms=[\d.]+; gpu_bytes=36000000; source CPU and GPU final zero', text)) == 1
                and text.count('model swap worker pressure: refusals=12; source CPU released; swap retry exact') == 1
                and all(text.count(f'model swap worker: {source}; SDR/HDR exact; source CPU/GPU credit zero') == 1
                        for source in ['triangle.stl', 'obj-shared-metadata.obj', 'off-concave-u.off',
                                       'ply-32-ascii.ply', 'ply-offset64-be.ply', 'ply-u64-le.ply'])
                and text.count('model upload bounded queue: first active job blocked; 1000 replacements; one latest pending; obsolete active discarded') == 1
                and text.count('model upload source ownership: managed-source exact; lease prevents eviction; cancellation/shutdown releases source CPU credit') == 1
                and text.count('model upload completed races: generation-change and explicit-cancel release queued GPU credit') == 1
                and text.count('model upload worker: 1000 replacements; latest only; generation cancellation; final credit zero') == 1)
            row['verified_execution'] = row['verified_execution'] and row['workload_evidence_valid']
        if name == 'gpu-tiff-viewer-pages':
            row['workload_evidence_valid'] = (
                row['passed'] == 1 and row['ignored'] == 0
                and text.count('TIFF viewer pages adapter: Metal ') == 1
                and text.count('Selected container stale prefetch tiff-distinct-page-profiles.tif: three obsolete indices refused; swap reads=0; RAM entries=0; shared credit=0; fresh generation retry follows') == 1
                and text.count('Selected container planned prefetch tiff-distinct-page-profiles.tif: backward2 -> [1,0]; forward1 -> [2]; swap bypasses blocked decoder; final=0') == 1
                and text.count('TIFF viewer pages: three independent selected keys; full-pressure RAM hits; nonzero TTL; blocked decoder reverse prefetch; payload and SDR/HDR exact; final=0') == 1)
            row['verified_execution'] &= row['workload_evidence_valid']
        if name == 'gpu-ico-viewer-pages':
            row['workload_evidence_valid'] = (
                row['passed'] == 1 and row['ignored'] == 0
                and text.count('ICO/CUR viewer adapter: Metal ') == 1
                and all(text.count(f'Selected container stale prefetch {source}: three obsolete indices refused; swap reads=0; RAM entries=0; shared credit=0; fresh generation retry follows') == 1
                        for source in ['ico-selected-images.ico','cur-selected-images.cur','cur-selected-dib.cur'])
                and all(text.count(f'Selected container planned prefetch {source}: backward2 -> [1,0]; forward1 -> [2]; swap bypasses blocked decoder; final=0') == 1
                        for source in ['ico-selected-images.ico','cur-selected-images.cur','cur-selected-dib.cur'])
                and all(text.count(f'ICO/CUR viewer {source}: selected keys; pressure RAM hits; TTL; blocked decoder reverse prefetch; payload/hotspot/SDR/HDR exact; final=0') == 1
                        for source in ['ico-selected-images.ico','cur-selected-images.cur','cur-selected-dib.cur']))
            row['verified_execution'] &= row['workload_evidence_valid']
        if name == 'gpu-ico-pages':
            row['workload_evidence_valid'] = (
                row['passed'] == 5 and row['ignored'] == 0
                and text.count('ICO pages adapter: Metal ') == 1
                and text.count('CUR pages adapter: Metal ') == 1
                and text.count('CUR DIB pages adapter: Metal ') == 1
                and text.count('CUR DIB depths pages adapter: Metal ') == 5
                and text.count('CUR patterned adapter: Metal ') == 1
                and all(text.count(f'CUR patterned depth{depth}: 9x3 packed rows/mixed mask/alpha; native oracle and SDR/HDR swap exact; final=0') == 1 for depth in [1,4,8,16,24,32])
                and all(text.count(f'CUR DIB depth{depth}: palette/alpha/mask; matching hotspots; RAM TTL reverse swap; SDR/HDR exact; final=0') == 1 for depth in [1,4,8,16,32])
                and text.count('CUR DIB pages: padded24bit AND masks; matching hotspots; RAM TTL reverse swap; SDR/HDR exact; final=0') == 1
                and text.count('CUR pages: quality order3; matching hotspots; exact RGBA alpha; count1/64byte RAM; full-pressure hit; visible TTL; reverse swap; SDR/HDR exact; final=0') == 1
                and text.count('ICO pages: quality order3; exact RGBA alpha; count1/64byte RAM; full-pressure hit; visible TTL; reverse swap; SDR/HDR exact; final=0') == 1)
            row['verified_execution'] &= row['workload_evidence_valid']
        if name == 'gpu-tiff-pages':
            names = [f'precision-{kind}-{order}-{change}.tif'
                     for kind in ['uint16', 'float32'] for order in ['little', 'big']
                     for change in ['same', 'changed']]
            row['workload_evidence_valid'] = (
                row['passed'] == 4 and row['ignored'] == 0
                and text.count('TIFF pages adapter: Metal ') == 1
                and text.count('TIFF precision adapter: Metal ') == 1
                and text.count('TIFF signed HDR adapter: Metal ') == 1
                and text.count('TIFF page ICC adapter: Metal ') == 1
                and text.count('TIFF page ICC: sRGB->linear->sRGB; identical source codes; LittleCMS RGB8 within1; fallback override refused; reverse swap SDR/HDR exact; final=0') == 1
                and all(text.count(f'TIFF signed HDR: {order}; above-one negative signed-zero; straight/associated alpha; orientation6; pressure/cancel retry; native bits and SDR/HDR exact; final=0') == 1
                        for order in ['le', 'be'])
                and all(text.count(f'TIFF pages: {variant}; selected metadata; visible TTL pin; full-pressure RAM hit; reverse swap; SDR/HDR exact; final=0') == 1
                        for variant in ['classic-le', 'classic-be', 'big-le', 'big-be'])
                and all(text.count(f'TIFF precision: {source}; native samples exact; ICC selected metadata; reverse swap; SDR/HDR exact; final=0') == 1
                        for source in names))
            row['verified_execution'] &= row['workload_evidence_valid']
        if name == 'gpu-png-physical-aspect':
            row['workload_evidence_valid'] = (
                row['passed'] == 5 and row['ignored'] == 0
                and 'APNG previous adapter: Metal ' in text
                and 'APNG previous rectangle: three selected disposal frames; independent SDR/HDR oracle exact; final=0' in text
                and 'APNG packed adapter: Metal ' in text
                and 'APNG packed precision: six selected frames; grayscale/palette alpha; SDR/HDR and swap exact; final=0' in text
                and 'APNG precision adapter: Metal ' in text
                and 'APNG 16-bit precision: two selected frames; authored linear SDR/HDR reference exact; final=0' in text
                and 'PNG aspect adapter: Metal ' in text
                and 'APNG aspect adapter: Metal ' in text
                and 'PNG physical aspect: width/height=0.5; decoded and swapped SDR/HDR frames exact; final=0' in text
                and 'APNG physical aspect: two selected frames; reverse swap restore; SDR/HDR exact; final=0' in text)
            row['verified_execution'] &= row['workload_evidence_valid']
        if name == 'gpu-large-apng':
            row['workload_evidence_valid'] = (
                row['passed'] == 1 and row['ignored'] == 0
                and text.count('large APNG adapter: Metal ') == 1
                and text.count('large APNG: 4000x3000; all48000000 samples; pressure retry; SDR/HDR and swap exact; final=0') == 1
                and len(re.findall(r'large APNG managed decode peak: [1-9]\d* bytes', text)) == 1)
            row['verified_execution'] &= row['workload_evidence_valid']
        if name == 'gpu-compute-cache':
            row['workload_evidence_valid'] = compute_cache_evidence(text, row['passed'], row['ignored'])
            row['verified_execution'] = row['verified_execution'] and row['workload_evidence_valid']
        if name == 'gpu-real-raw-collection':
            row['workload_evidence_valid'] = raw_collection_evidence(
                text, args.metal_raw_collection_seconds, len(external_inputs['raw_collection']))
            row['verified_execution'] = row['verified_execution'] and row['workload_evidence_valid']
        if name == 'gpu-mixed-raw-worker':
            expected_paths = [item['path'] for item in external_inputs['raw_collection']]
            hdr_paths = re.findall(r'^mixed RAW worker HDR: (.+); [1-9]\d*x[1-9]\d*; every scene-linear binary16 component exact and finite$', text, re.MULTILINE)
            row['workload_evidence_valid'] = (
                row['passed'] == 1 and row['ignored'] == 0
                and 'mixed RAW worker adapter: Metal ' in text
                and text.count('original samples and frame exact; disk-count=1; root=0') == len(expected_paths)
                and hdr_paths == expected_paths)
            row['verified_execution'] = row['verified_execution'] and row['workload_evidence_valid']
        if name == 'gpu-multi-neighbor-raw-worker':
            expected_paths = [external_inputs['raw_collection'][index]['path'] for index in (3, 4, 1, 1, 0, 3)]
            hdr_paths = re.findall(r'^multi RAW worker HDR: (.+); [1-9]\d*x[1-9]\d*; every scene-linear binary16 component exact and finite$', text, re.MULTILINE)
            row['workload_evidence_valid'] = (
                row['passed'] == 1 and row['ignored'] == 0
                and 'multi RAW worker adapter: Metal ' in text
                and hdr_paths == expected_paths
                and text.count('samples/metadata and 1920x1080 Metal frame exact; root=0') == 6
                and len(re.findall(r'multi RAW full frame: [1-9]\d*x[1-9]\d*; samples/metadata and every Metal pixel exact; root_before_release=\d+', text)) == 6
                and text.count('three neighbors complete; disk-count=3; failures=0') == 2)
            row['verified_execution'] = row['verified_execution'] and row['workload_evidence_valid']
        if name == 'gpu-raster-collection':
            row['workload_evidence_valid'] = raster_collection_evidence(text)
            row['verified_execution'] = row['verified_execution'] and row['workload_evidence_valid']
        if not row['verified_execution']:
            print(f'{name}: failed or no passing test execution was recorded', flush=True)
        results.append(row)
        report = {'gpu_backend': args.gpu_backend, 'gpu_optional': False, 'storage_soak_seconds_per_mode': args.storage_soak_seconds, 'gpu_policy_cycles': args.metal_policy_cycles, 'large_hdr_policy_cycles': args.metal_large_policy_cycles, 'metal_collection_seconds': args.metal_collection_seconds, 'metal_raw_collection_seconds': args.metal_raw_collection_seconds, 'metal_raw_collection_manifest': str(args.metal_raw_collection_manifest) if args.metal_raw_collection_manifest else None, 'metal_prefetch_navigation': args.metal_prefetch_navigation, 'metal_raw_worker_navigation': args.metal_raw_worker_navigation, 'metal_large_float_hdr': args.metal_large_float_hdr, 'metal_viewer_ttl_prefetch': args.metal_viewer_ttl_prefetch, 'metal_large_upload_cancel': args.metal_large_upload_cancel, 'metal_compute_cache': args.metal_compute_cache, 'metal_model_swap_cycles': args.metal_model_swap_cycles, 'large_hdr_policy': bool(args.metal_policy_cycles), 'full_sensor_compute': args.full_sensor_compute, 'suites': results, 'complete_suite_execution': len(results) == len(suites),
                  'limitations': ['Passing host CUDA tests does not qualify NVIDIA hardware.',
                                  'Metal readback does not qualify physical HDR display.',
                                  'Ignored tests are unverified, not passing.',
                                  'These regression suites do not prove all 100 formats or sustained slow-disk workloads.']}
        report['invocation'] = [sys.executable, str(Path(__file__).resolve()), *sys.argv[1:]]
        report['requested_options'] = {
            name: str(value) if isinstance(value, Path) else value
            for name, value in vars(args).items()
        }
        report['metal_model_upload_worker'] = args.metal_model_upload_worker
        report['metal_swapped_residency_required'] = args.gpu_backend == 'metal' and not args.gpu_only
        report['ai_native_metal_transport'] = bool(args.ai_native_root)
        report['ai_ram_swap_metal'] = bool(args.ai_native_root) and not args.gpu_only
        if args.ai_native_root:
            report['limitations'].append('AI transport uses pinned test-only replay planes; it does not qualify production AI decoding, original appearance or physical HDR.')
        report['mef_full_sensor_metal'] = bool(args.mef_sensor_source)
        if args.mef_sensor_source:
            report['limitations'].append(
                'MEF tests original sensor transport through authored channels, not Mamiya WB, demosaic, color or viewer support.')
        report['metal_large_lod'] = args.metal_large_lod
        report['metal_raster_timing'] = args.metal_raster_timing
        report['metal_hdr_range'] = args.metal_hdr_range
        report['metal_raster_collection'] = args.metal_raster_collection
        report['metal_linear_import'] = args.metal_linear_import
        if args.metal_raster_collection:
            report['limitations'].append(
                'Raster collection checks 118 fixture sources in native/prepared storage, not independent decoder or color correctness, all format variants or physical HDR.')
        if args.storage_soak_seconds:
            report['limitations'].append(
                'Storage soak uses delayed test I/O on the local disk; it does not qualify a physical slow disk.')
        if args.metal_policy_cycles:
            report['limitations'].append(
                'Large HDR policy checks one authored 12 MP payload cycle and offscreen SDR output, not physical HDR or a collection soak.')
        if args.metal_large_policy_cycles:
            report['limitations'].append(
                'Repeated authored 12 MP frames qualify bounded local-disk cycles, not diverse format collections, hours of operation or physical HDR.')
        if args.metal_collection_seconds:
            report['limitations'].append(
                'Long-lived collection uses eight authored float rasters and local disk; it does not qualify diverse source decoding, a physical slow disk or HDR display.')
        if args.full_sensor_compute:
            report['limitations'].append(
                'Full-sensor compute checks exposure/readback of authored float pixels; it does not qualify every RAW demosaic pipeline.')
        report['metal_required_viewer_cases'] = list(metal_cases)
        report['external_viewer_inputs'] = external_inputs
        report['metal_mixed_raw_workers'] = args.metal_mixed_raw_workers
        report['all_executed_suites_verified'] = all(r['verified_execution'] for r in results)
        (output / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    return 1 if any(not r['verified_execution'] for r in results) else 0

if __name__ == '__main__':
    raise SystemExit(main())
