"""Fetch pinned public W3C PNG attachments and produce native-code FFmpeg oracles.
Usage: python3 scripts/fetch-w3c-hdr-png-conformance.py /absolute/output/directory
No color conversion policy is applied by this native RGB16 oracle.
"""
import hashlib
import json
import pathlib
import subprocess
import sys
import urllib.request

root = pathlib.Path(sys.argv[1]).resolve()
root.mkdir(parents=True, exist_ok=True)
files = {
    'PNG-HLG-FancyColorBars-16bit-CICP-FR.png': '26617b8ce1f3cea8df91200ad5fd86bde11cde6d94cefa84a988c3e4d3b3bb2a',
    'PNG-HLG-FancyColorBars-16bit-CICP-NR.png': '480bf843742c2d966bbab039fac7a6d4104c874ed1ae73613aca1b81fe2a4ded',
    'PNG-PQ-BT.2111-ColorBars-16bit-CICP-FR.png': 'ac31eb3552c1bafc28b29bc9d0d50723582ef911c4ea293495eb5320b2db4dac',
}
manifest = []
validated = []
# Verify the whole input set before replacing any prior oracle.
for name, expected in files.items():
    url = 'https://lists.w3.org/Archives/Public/public-png/2023Dec/att-0031/' + name
    source = root / name
    if source.exists():
        data = source.read_bytes()
    else:
        with urllib.request.urlopen(url, timeout=30) as response:
            data = response.read(64 * 1024 * 1024 + 1)
    if len(data) > 64 * 1024 * 1024 or hashlib.sha256(data).hexdigest() != expected:
        raise ValueError('source size/hash mismatch (retained file is not replaced): ' + name)
    if not source.exists():
        source.write_bytes(data)
    validated.append((name, expected, url, source, len(data)))
for name, expected, url, source, source_bytes in validated:
    oracle = root / (name + '.rgb48le')
    command = ['ffmpeg', '-v', 'error', '-i', str(source), '-frames:v', '1', '-pix_fmt', 'rgb48le', '-f', 'rawvideo', '-y', str(oracle)]
    subprocess.run(command, check=True)
    reference = oracle.read_bytes()
    if len(reference) != 1920 * 1080 * 6:
        raise ValueError('unexpected reference extent: ' + name)
    manifest.append(dict(name=name, url=url, source_sha256=expected, source_bytes=source_bytes, oracle_sha256=hashlib.sha256(reference).hexdigest(), oracle_bytes=len(reference), command=command))
report = dict(ffmpeg_version=subprocess.check_output(['ffmpeg', '-version'], text=True).splitlines()[0], files=manifest)
(root / 'oracle-manifest.json').write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps(report, indent=2))
