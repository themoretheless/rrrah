#!/usr/bin/env python3
"""Prepare all original/strong Copydays images without generating test labels."""
import argparse
import hashlib
import io
import json
import pathlib
import tarfile
from PIL import Image, ImageOps

p = argparse.ArgumentParser()
p.add_argument('root', type=pathlib.Path)
p.add_argument('acquisition', type=pathlib.Path)
p.add_argument('protocol', type=pathlib.Path)
p.add_argument('output', type=pathlib.Path)
a = p.parse_args()
acquisition = json.loads(a.acquisition.read_text())
protocol = json.loads(a.protocol.read_text())
assert hashlib.sha256(pathlib.Path(protocol['path']).read_bytes()).hexdigest() == protocol['source_sha256']
images = {'original': [], 'strong': []}
for archive_record in acquisition['archives']:
    archive_path = pathlib.Path(archive_record['path'])
    assert hashlib.sha256(archive_path.read_bytes()).hexdigest() == archive_record['sha256']
    split = 'original' if archive_path.name == 'copydays_original.tar.gz' else 'strong'
    assert archive_path.name in ('copydays_original.tar.gz', 'copydays_strong.tar.gz')
    with tarfile.open(archive_path, 'r:gz') as archive:
        seen = set()
        for member in archive:
            path = pathlib.PurePosixPath(member.name)
            assert not path.is_absolute() and '..' not in path.parts
            assert member.isdir() or member.isfile()
            if member.isdir(): continue
            assert path.suffix.lower() == '.jpg'
            assert path.name not in seen
            seen.add(path.name)
            data = archive.extractfile(member).read()
            assert len(data) == member.size
            with Image.open(io.BytesIO(data)) as source:
                source.load()
                assert source.format == 'JPEG'
                source_size = list(source.size)
                image = ImageOps.exif_transpose(source).convert('RGB')
                image.thumbnail((320, 320), Image.Resampling.LANCZOS)
                target = a.root / 'normalized' / split / (path.stem + '.png')
                target.parent.mkdir(parents=True, exist_ok=True)
                image.save(target)
                images[split].append({
                    'filename': path.name, 'source_member': member.name,
                    'source_archive_sha256': archive_record['sha256'],
                    'source_sha256': hashlib.sha256(data).hexdigest(),
                    'source_size': source_size,
                    'normalized_path': str(target.resolve()),
                    'normalized_size': list(image.size),
                    'normalized_sha256': hashlib.sha256(target.read_bytes()).hexdigest(),
                    'group_id': int(path.stem) // 100 * 100,
                })
assert len(images['original']) == 157 and len(images['strong']) == 229
originals = {v['group_id']: v for v in images['original']}
assert len(originals) == 157
assert all(int(v['filename'][:-4]) == v['group_id'] for v in images['original'])
pairs = []
for query in sorted(images['strong'], key=lambda v: v['filename']):
    assert query['group_id'] in originals
    assert int(query['filename'][:-4]) != query['group_id']
    pairs.append({'query_id': query['filename'], 'left': originals[query['group_id']],
                  'right': query, 'label': 'publisher_origin_copy'})
result = {'original_images': 157, 'strong_images': 229, 'required_positive_pairs': 229,
          'normalization': 'EXIF transpose, RGB, Lanczos maximum side 320; no upsampling',
          'protocol_reference': protocol, 'images': images, 'positive_pairs': pairs,
          'scope': 'Filename origin groups per pinned reference protocol. Copy labels do not imply exact pixel equality. No unrelated-pair labels or performance qualification yet.'}
a.output.write_text(json.dumps(result, indent=2) + '\n')
print({k: result[k] for k in ('original_images', 'strong_images', 'required_positive_pairs')})
