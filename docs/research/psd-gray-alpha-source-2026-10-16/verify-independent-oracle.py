#!/usr/bin/env python3
"""Test-only psd-tools verification; run with the isolated oracle environment."""
from pathlib import Path
from importlib.metadata import version
import hashlib
import json
from psd_tools import PSDImage

folder = Path(__file__).resolve().parent
fixtures = folder.parents[2] / 'tests/fixtures/raster'
records = {Path(row['name']).name: row for row in json.loads((folder/'inventory.json').read_text())}
expected = (fixtures/'psd-tools-gray0.rgba').read_bytes()
expected_profile = (fixtures/'psd-tools-gray0.icc').read_bytes()
assert len(expected) == 400*359*4
cases = []
for suffix in ['psd', 'psb']:
    name = 'gray0.' + suffix
    source = fixtures/('psd-tools-'+name)
    digest = hashlib.sha256(source.read_bytes()).hexdigest()
    assert digest == records[name]['sha256'], 'original source identity changed'
    image = PSDImage.open(source)
    rgba = image.topil(apply_icc=False).convert('RGBA').tobytes()
    assert rgba == expected, 'independent straight-alpha pixels differ'
    assert image.image_resources.get_data(1039) == expected_profile, 'ICC resource differs'
    alpha = rgba[3::4]
    partial = sum(0 < value < 255 for value in alpha)
    assert partial > 0
    cases.append({'fixture':source.name,'source_sha256':digest,
        'rgba_sha256':hashlib.sha256(rgba).hexdigest(),'pixels':400*359,
        'different_channels':0,'icc_bytes':len(expected_profile),
        'alpha_zero':alpha.count(0),'alpha_opaque':alpha.count(255),
        'alpha_partial':partial,'distinct_alpha_codes':len(set(alpha))})
report = {'psd_tools_version':version('psd-tools'),'attrs_version':version('attrs'),
    'operation':'topil(apply_icc=False).convert(RGBA)','cases':cases,
    'scope':'independent original/native-reference qualification; does not execute Rust or Metal'}
(folder/'reproduced-independent-oracle.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps(report,indent=2))
