#!/usr/bin/env python3
"""Measure full-image GPR interpolation differences; this is not a pass/fail format gate.

Requires NumPy. Inputs: raw_developed_dump output CAMERA.rgba32fle and independent
Adobe DNG SDK Stage3 TIFF CAMERA-stage3.tif for each pinned camera fixture.
"""
import argparse
import hashlib
import json
import struct
from pathlib import Path
import numpy as np

ROOT = Path(__file__).resolve().parents[1]

def stage3(path):
    data = path.read_bytes()
    order = '<' if data[:2] == b'II' else '>' if data[:2] == b'MM' else None
    if order is None or struct.unpack_from(order+'H', data, 2)[0] != 42:
        raise ValueError('expected classic TIFF')
    offset = struct.unpack_from(order+'I', data, 4)[0]
    count = struct.unpack_from(order+'H', data, offset)[0]
    tags = {}
    for i in range(count):
        at = offset+2+i*12
        tag, kind, n, location = struct.unpack_from(order+'HHII', data, at)
        size = {3: 2, 4: 4}.get(kind)
        if size:
            pos = at+8 if size*n <= 4 else location
            tags[tag] = struct.unpack_from(order+str(n)+('H' if kind == 3 else 'I'), data, pos)
    if (tags.get(259) != (1,) or tags.get(258) != (16,16,16)
            or tags.get(277) != (3,) or tags.get(284, (1,)) != (1,)):
        raise ValueError('expected uncompressed interleaved RGB16 Stage3')
    width, height = tags[256][0], tags[257][0]
    chunks = []
    for pos, size in zip(tags[273], tags[279], strict=True):
        if pos+size > len(data):
            raise ValueError('truncated TIFF strip')
        chunks.append(data[pos:pos+size])
    payload = b''.join(chunks)
    if len(payload) != width*height*6:
        raise ValueError('unexpected Stage3 payload size')
    return np.frombuffer(payload, dtype=order+'u2').reshape(height,width,3)

def source_orientation(path):
    data = path.read_bytes()
    order = '<' if data[:2] == b'II' else '>' if data[:2] == b'MM' else None
    if order is None or struct.unpack_from(order+'H',data,2)[0] != 42:
        raise ValueError('expected classic SDK-converted DNG')
    offset = struct.unpack_from(order+'I',data,4)[0]
    count = struct.unpack_from(order+'H',data,offset)[0]
    for i in range(count):
        at = offset+2+i*12
        tag,kind,n,_ = struct.unpack_from(order+'HHII',data,at)
        if tag == 274:
            if kind != 3 or n != 1:
                raise ValueError('invalid orientation tag')
            return struct.unpack_from(order+'H',data,at+8)[0]
    return 1

def analyze(folder, dng_folder, fixture):
    name = fixture['camera']
    reference_path = folder / (name+'-stage3.tif')
    native_path = folder / (name+'.rgba32fle')
    reference = stage3(reference_path)
    orientation = source_orientation(dng_folder / (name+'.dng'))
    if orientation == 2:
        reference = reference[:,::-1]
    elif orientation != 1:
        raise ValueError(f'{name}: unsupported comparison orientation {orientation}')
    h,w,_ = reference.shape
    if native_path.stat().st_size != h*w*16:
        raise ValueError(f'{name}: native/SDK sensor dimensions disagree')
    native = np.memmap(native_path, dtype='<f4', mode='r', shape=(h,w,4))
    for y in range(0,h,128):
        rows = native[y:y+128]
        if not np.isfinite(rows).all() or not (rows[:,:,3] == 1).all():
            raise ValueError(f'{name}: invalid native linear pixels/alpha')
    matrix = np.array(fixture['independent_matrix'])
    measurements = {}
    for size in [8,32,64]:
        total = np.zeros(3); absolute = np.zeros(3); count = 0
        for y in range(size,h-size,size):
            # Vectorized horizontal block means; exclude boundary blocks.
            end = ((w-size)//size)*size
            actual = native[y:y+size,size:end,:3].reshape(size,-1,size,3).mean(axis=(0,2),dtype=np.float64)
            camera = reference[y:y+size,size:end].reshape(size,-1,size,3).mean(axis=(0,2),dtype=np.float64)/65535
            difference = actual-camera@matrix.T
            total += difference.sum(axis=0)
            absolute += np.abs(difference).sum(axis=0)
            count += len(difference)
        measurements[str(size)] = {'blocks': count,'signed_mean_rgb': (total/count).tolist(),
                                   'mean_abs_rgb': (absolute/count).tolist()}
    def digest(path):
        result = hashlib.sha256()
        with path.open('rb') as stream:
            for chunk in iter(lambda: stream.read(1024*1024), b''):
                result.update(chunk)
        return result.hexdigest()
    return {'camera':name,'dimensions':[w,h],'all_pixels_finite_opaque':True,
            'pixels_checked':w*h,'source_orientation':orientation,'native_sha256':digest(native_path),
            'independent_stage3_sha256':digest(reference_path),'linear_block_means':measurements}

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--input-dir',type=Path,required=True)
    parser.add_argument('--dng-dir',type=Path,required=True,help='Independent SDK-converted DNGs, used for source orientation')
    parser.add_argument('--output-report',type=Path,required=True)
    args = parser.parse_args()
    fixtures = json.loads((ROOT/'crates/rrrah-decode/tests/fixtures/gpr-color-matrices.json').read_text())
    report = {'scope':'Full native scene-linear AHD vs independent SDK bilinear Stage3 transformed by independent camera matrices',
              'limitations':['Block means characterize interpolation differences; no perceptual quality or universal format pass is inferred.',
                             'The reference uses RGB16 quantization. Boundary blocks are excluded from mean comparisons.'], 'rows':[]}
    for fixture in fixtures:
        row = analyze(args.input_dir,args.dng_dir,fixture); report['rows'].append(row)
        print(row['camera'],row['linear_block_means']['32'],flush=True)
    args.output_report.write_text(json.dumps(report,indent=2)+'\n')

if __name__ == '__main__':
    main()
