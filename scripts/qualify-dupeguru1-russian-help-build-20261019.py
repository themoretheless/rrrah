"""Build original Russian help on private copies and inspect conditional content."""
from pathlib import Path
import hashlib
import json
import os
import shutil
import subprocess
import sys

R = Path(__file__).resolve().parents[1]
S = R / '.dedup-work/dupeguru1-20261019/source'
W = R / '.dedup-work/dupeguru1-russian-help-build-20261019'
H = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()


def main():
    W.mkdir(exist_ok=False)
    (W / 'help').mkdir()
    shutil.copytree(S / 'help/ru', W / 'help/ru')
    for name in ['changelog', 'changelog.tmpl', 'conf.tmpl']:
        shutil.copy2(S / 'help' / name, W / 'help' / name)
    names = ['build.py', 'hscommon/sphinxgen.py', 'help/changelog', 'help/changelog.tmpl', 'help/conf.tmpl']
    names += [str(p.relative_to(S)) for p in sorted((S / 'help/ru').glob('*.rst'))]
    pins = {name: H(S / name) for name in names}
    command = [sys.executable, '-c', 'import build; build.build_one_help("ru")']
    with (W / 'build.log').open('wb') as log:
        completed = subprocess.run(command, cwd=W, env={**os.environ, 'PYTHONPATH': str(S)}, stdout=log, stderr=subprocess.STDOUT)
    assert completed.returncode == 0
    output = W / 'build/help/ru/preferences.html'
    html = output.read_text()
    source = (S / 'help/ru/preferences.rst').read_text(encoding='utf-8-sig')
    assert 'Аудио контента' in source and 'Аудио контента' not in html
    assert 'Специальной команды' in html
    assert all(H(S / name) == pin for name, pin in pins.items())
    report = {'status': 'original_Russian_help_build_finished_conditional_sections_absent',
              'exit_code': completed.returncode, 'command': command, 'source_pins': pins,
              'build_log_sha256': H(W / 'build.log'), 'preferences_html_sha256': H(output),
              'checks': {'edition_specific_audio_section_present_in_source': True,
                         'edition_specific_audio_section_absent_from_HTML': True,
                         'shared_custom_command_section_present_in_HTML': True},
              'limits': ['Original build_one_help and sphinxgen used on task-owned copied help; source unchanged.',
                         'Sphinx8.2.3 output retains warnings/errors in log; wrapper return alone is not a warning-free build gate.',
                         'HTML content inspected programmatically, not visually rendered or tested as live application help.',
                         'No original upstream tests, user file actions or production changes.']}
    (W / 'result.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'status': report['status'], 'exit_code': completed.returncode}))


if __name__ == '__main__':
    main()
