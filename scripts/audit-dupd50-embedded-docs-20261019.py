"""Verify complete generated representations against the reviewed text and actual CLI."""
from pathlib import Path
import hashlib
import json
import os
import re
import subprocess

R = Path(__file__).resolve().parents[1]
S = R / '.dedup-work/dupd50-20261019/source'
W = R / '.dedup-work/dupd50-embedded-docs-v2-20261019'
h = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()

def main():
    assert subprocess.check_output(['git', '-C', str(S), 'rev-parse', 'HEAD'], text=True).strip() == '33907bbcfd5fe8b63f54d4f55438fa222ea880aa'
    W.mkdir(exist_ok=False)
    pins = {}
    for name in ['man/dupd', 'COPYING', 'src/copying.h', 'src/copying.c', 'src/man.h', 'src/main.c', 'version']:
        raw = (S / name).read_bytes()
        assert subprocess.check_output(['git', '-C', str(S), 'show', 'HEAD:' + name]) == raw
        pins[name] = h(S / name)
    man_source = (S / 'src/man.h').read_text()
    match = re.fullmatch(r'unsigned char man_dupd\[\] = \{\s*((?:0x[0-9a-f]{2}(?:,\s*|\s*))+)\};\s*unsigned int man_dupd_len = (\d+);\s*', man_source)
    assert match
    man = bytes(int(x, 16) for x in re.findall(r'0x([0-9a-f]{2})', match[1]))
    assert len(man) == int(match[2]) and man == (S / 'man/dupd').read_bytes()
    code = re.sub(r'/\*.*?\*/', '', (S / 'src/copying.c').read_text(), flags=re.S)
    for include in ['#include <stdio.h>', '#include <stdlib.h>', '#include "copying.h"']:
        assert code.count(include) == 1
        code = code.replace(include, '')
    match = re.fullmatch(r'\s*void show_license\(\)\s*\{(.*)\}\s*', code, flags=re.S)
    assert match
    body = match[1]
    literal = r'printf\(("(?:[^"\\\n]|\\.)*")\);'
    strings = re.findall(literal, body)
    assert not re.sub(literal, '', body).strip()
    assert all('%' not in x for x in strings)
    license_text = ''.join(json.loads(x) for x in strings).encode()
    assert license_text.startswith((S / 'COPYING').read_bytes())
    suffix = license_text[len((S / 'COPYING').read_bytes()):]
    (W / 'license-suffix.txt').write_bytes(suffix)
    binary = S / 'dupd'
    pin = h(binary)
    home = W / 'home'; home.mkdir()
    rows = []
    banner = ('dupd ' + (S / 'version').read_text().strip() + ' Copyright 2012-2020 Jyri J. Virkki\n'
              'This program comes with ABSOLUTELY NO WARRANTY.\n'
              'This is free software, and you are welcome to redistribute it\n'
              "under certain conditions. Run 'dupd license' for details.\n\n").encode()
    for command, expected in [('man', banner + man), ('license', license_text)]:
        result = subprocess.run([str(binary), command], env={**os.environ, 'HOME': str(home)}, capture_output=True)
        (W / (command + '.stdout')).write_bytes(result.stdout)
        (W / (command + '.stderr')).write_bytes(result.stderr)
        (W / (command + '.exit.json')).write_text(json.dumps({'exit_code': result.returncode}) + '\n')
        assert result.returncode == 0 and result.stdout == expected and not result.stderr
        rows.append({'command': command, 'exit_code': result.returncode, 'bytes': len(result.stdout), 'stdout_sha256': hashlib.sha256(result.stdout).hexdigest()})
    assert h(binary) == pin and all(h(S / f) == pin_value for f, pin_value in pins.items())
    result = {'status': 'verified_complete_generated_representation_and_cli_output', 'source_pins': pins, 'binary_sha256': pin, 'commands': rows, 'license_suffix_sha256': hashlib.sha256(suffix).hexdigest(), 'limits': ['Generated sources decoded and structurally checked completely, not claimed line-by-line manual reading.', 'Manual GPL and rendered man text reading recorded separately; this is not legal advice or full repository runtime coverage.']}
    (W / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result))

if __name__ == '__main__':
    main()
