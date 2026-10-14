#!/usr/bin/env python3
"""Bounded original-shell fragment controls; never invoke upstream deletion."""
from pathlib import Path
import hashlib
import json
import re
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
E = ROOT / 'docs/research/dedup-500/evidence/DRRDietrich--DeDup-Image'
OUT = ROOT / 'docs/research/dedup-shell-image248-controls-20261014.json'
assert not OUT.exists()
names = {'LICENSE':'01-LICENSE','README.md':'00-README.md','dedup-img':'02-dedup-img'}
tree = json.loads((E / 'tree.json').read_text())
for item in tree['tree']:
    data = (E / names[item['path']]).read_bytes()
    assert hashlib.sha1(b'blob ' + str(len(data)).encode() + b'\0' + data).hexdigest() == item['sha']
source = (E / '02-dedup-img').read_text()
helper = re.search(r"-exec bash -c '(.*?)' \\\;", source).group(1)
extract = '\n'.join(line for line in source.splitlines() if line.startswith('do sha=') or line.startswith('path='))
extract = extract.replace('do sha=', 'sha=', 1)
checks = []

def check(name, condition):
    assert condition, name
    checks.append(name)

check('original_bash_syntax_valid', subprocess.run(['bash','-n',str(E / '02-dedup-img')], capture_output=True).returncode == 0)
hash_value = 'a' * 128
run = subprocess.run(['sort','-k1,2r'], input=f'{hash_value}\t9\tsmall\n{hash_value}\t100\tlarge\n', capture_output=True, text=True, check=True)
check('reverse_size_sort_is_lexical_and_prefers_nine_over_one_hundred', run.stdout.splitlines()[0].endswith('\tsmall'))
for name, path, expected in [('spaces','folder/a b.png','folder/a b.png'),('tab','folder/a\tb.png','folder/a')]:
    row = hash_value + '\t9\t' + path
    run = subprocess.run(['bash','-c',extract + '\nprintf "%s" "$path"', 'control'], env={'PATH':'/usr/bin:/bin:/sbin','f':row}, capture_output=True, text=True, check=True)
    check('path_field_' + name, run.stdout == expected)
with tempfile.TemporaryDirectory(prefix='rrrah-image248-', dir='/private/tmp') as tmp:
    directory = Path(tmp)
    for name, filename, marker in [('quote','quote"; printf canary > quote_marker; #.png','quote_marker'),('substitution','$(printf canary > substitution_marker).png','substitution_marker')]:
        path = directory / filename
        path.write_bytes(b'synthetic name control')
        run = subprocess.run(['bash','-c',helper.replace('{}', str(path))], cwd=directory, capture_output=True, text=True)
        check('filename_' + name + '_executes_only_test_marker_command', (directory / marker).read_bytes() == b'canary')
    row = hash_value + '\t9\tfolder/first\nsecond.png'
    run = subprocess.run(['bash','-c',"IFS=$'\\n'; n=0; for f in $(printf '%s' \"$record\"); do n=$((n+1)); done; printf '%s' \"$n\""], env={'PATH':'/usr/bin:/bin:/sbin','record':row}, capture_output=True, text=True, check=True)
    check('newline_filename_splits_one_record_into_two', run.stdout == '2')
OUT.write_text(json.dumps(dict(status='seven_original_shell_fragment_controls_confirmed',repository='DRRDietrich/DeDup-Image',commit='3ba76e268c2226fd1478083797a72f456ac761f6',checks=checks,count=len(checks),pins={str(E / local):hashlib.sha256((E / local).read_bytes()).hexdigest() for local in names.values()},limits='Original bash syntax and selected exact helper/path/sort/IFS fragments only. All source-deletion code excluded. Two harmless filename controls write only temporary local marker files. No exiftool, GNU find/stat/uniq pipeline, actual metadata stripping, image identity, deletion or full CLI qualification.'),indent=2)+'\n')
print(f'{len(checks)} controls confirmed')
