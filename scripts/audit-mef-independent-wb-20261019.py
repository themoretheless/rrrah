#!/usr/bin/env python3
"""Read pinned reference source metadata; never execute a downloaded converter."""
from pathlib import Path
import hashlib
import json
import urllib.request

sources = {
 'legacy_dng': 'https://raw.githubusercontent.com/kristoffer-trolle/LegacyDenoiser/1b346c05524e21c4c784406dd49991196824a151/src/legacy_denoiser/dng.py',
 'legacy_camera': 'https://raw.githubusercontent.com/kristoffer-trolle/LegacyDenoiser/1b346c05524e21c4c784406dd49991196824a151/src/legacy_denoiser/cameras.py',
 'sample_instructions': 'https://raw.githubusercontent.com/pgray/fpexif/c4b85b9bd100f1717a64b9f18df26c4453100418/test-data/download-samples.sh',
}
result = {'scope': 'Pinned source inspection, not converter execution or proof of general absence of MEF white balance', 'sources': {}}
for name, url in sources.items():
    data = urllib.request.urlopen(url, timeout=30).read()
    text = data.decode()
    result['sources'][name] = {'url': url, 'sha256': hashlib.sha256(data).hexdigest(), 'bytes': len(data)}
    if name == 'legacy_dng':
        checks = {
            'reads_camera_wb': 'r.camera_whitebalance[:3]' in text,
            'requires_positive_components': 'np.all(cam_mul > 0)' in text,
            'fallback_uses_d65': 'neutral = color_matrix @ _XYZ_D65' in text,
            'writes_neutral_to_dng': '(50728, 5, 3, _rationals(as_shot_neutral), True)' in text,
        }
        assert all(checks.values())
        result['sources'][name]['observations'] = checks
    if name == 'legacy_camera':
        assert 'extensions=(".mef",)' in text
    if name == 'sample_instructions':
        result['sources'][name]['observations'] = {'download_instructions_only': 'you\'ll need to visit the site to get actual download URLs' in text}
result['conclusion'] = 'LegacyDenoiser daylight fallback cannot serve as independent camera-as-shot WB for the pinned MEF with zero camera multipliers. fpexif download instructions supply no new MEF fixture URL. Keep default MEF color refusal; no runtime dependency added.'
root = Path(__file__).resolve().parents[1]
out = root / 'docs/research/mef-independent-wb-audit-2026-10-19'
out.mkdir(parents=True, exist_ok=True)
(out / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result, indent=2))
