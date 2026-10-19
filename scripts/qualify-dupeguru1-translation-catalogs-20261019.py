"""Fully parse pinned catalogs, compile private copies and check runtime strings."""
from pathlib import Path
from collections import Counter
import gettext
import hashlib
import io
import json
import shutil
import string
import subprocess
import sys
import polib

R = Path(__file__).resolve().parents[1]
S = R / '.dedup-work/dupeguru1-20261019/source'
W = R / '.dedup-work/dupeguru1-translation-catalogs-20261019'
H = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()


def fields(text):
    result = []
    automatic = 0
    for _, field, spec, conversion in string.Formatter().parse(text):
        if field is None:
            continue
        if field == '':
            field = str(automatic)
            automatic += 1
        result.append(field)
        if '{' in spec:
            result.extend(fields(spec))
    return sorted(result)


def main():
    W.mkdir(exist_ok=False)
    source_files = sorted((S / 'locale').rglob('*'))
    source_files = [p for p in source_files if p.is_file()]
    assert len(source_files) == 66
    pins = {str(p.relative_to(S)): H(p) for p in source_files}
    for p in source_files:
        assert subprocess.check_output(['git', '-C', str(S), 'show', 'HEAD:' + str(p.relative_to(S))]) == p.read_bytes()
    shutil.copytree(S / 'locale', W / 'catalogs')
    sys.path.insert(0, str(S))
    from hscommon.loc import compile_all_po
    compile_all_po(str(W / 'catalogs'))
    rows, anomalies = [], []
    totals = Counter()
    for p in source_files:
        catalog = polib.pofile(p, check_for_duplicates=True)
        assert not any(e.msgid_plural or e.msgctxt for e in catalog), 'plural/context needs explicit qualification'
        if p.suffix == '.po':
            compiled = W / 'catalogs' / p.relative_to(S / 'locale').with_suffix('.mo')
            runtime = gettext.GNUTranslations(io.BytesIO(compiled.read_bytes()))
        else:
            compiled = None
            runtime = gettext.GNUTranslations(io.BytesIO(catalog.to_binary()))
        counts = Counter()
        for entry in catalog:
            counts['entries'] += 1
            if entry.obsolete:
                counts['obsolete'] += 1
                continue
            counts['active'] += 1
            if 'fuzzy' in entry.flags:
                counts['fuzzy'] += 1
            translated = bool(entry.msgstr) and 'fuzzy' not in entry.flags
            expected = entry.msgstr if translated else entry.msgid
            assert runtime.gettext(entry.msgid) == expected, (p, entry.msgid)
            counts['runtime_roundtrips'] += 1
            if not translated:
                counts['fallbacks'] += 1
                continue
            counts['translated'] += 1
            try:
                before, after = fields(entry.msgid), fields(entry.msgstr)
            except ValueError as error:
                anomalies.append({'path': str(p.relative_to(S)), 'msgid': entry.msgid, 'msgstr': entry.msgstr, 'kind': 'brace_parse_error', 'error': str(error)})
                continue
            if before != after:
                anomalies.append({'path': str(p.relative_to(S)), 'msgid': entry.msgid, 'msgstr': entry.msgstr, 'kind': 'brace_field_mismatch', 'source_fields': before, 'translated_fields': after})
        totals.update(counts)
        rows.append({'path': str(p.relative_to(S)), 'sha256': H(p), 'bytes': p.stat().st_size,
                     'entries': dict(counts), 'encoding': catalog.encoding,
                     'MO_sha256': H(compiled) if compiled else None})
    assert all(H(S / n) == pin for n, pin in pins.items())
    report = {'status': 'all_catalogs_parsed_compiled_roundtripped_with_format_diagnostics',
              'polib_version': polib.__version__, 'catalogs': rows, 'totals': dict(totals),
              'brace_anomalies': anomalies, 'source_pins': pins,
              'compiler_source_sha256': H(S / 'hscommon/loc.py'),
              'limits': ['Every catalog entry parsed; all active singular strings checked against GNUTranslations after compilation.',
                         'Original compile_all_po invoked only on task-owned copies; original catalogs unchanged.',
                         'Brace fields compared structurally; percent formatting, translation semantics, RTL layout and actual UI not qualified.',
                         'Fallback is observed behavior, not automatically a defect; templates intentionally lack translations.',
                         'This is exhaustive structural/runtime catalog analysis, not manual reading or linguistic review of7667 entries.',
                         'No additional upstream tests, production changes, remote translation sync or user settings.']}
    (W / 'result.json').write_text(json.dumps(report, indent=2, ensure_ascii=False) + '\n')
    print(json.dumps({'catalogs': len(rows), 'totals': dict(totals), 'brace_anomalies': anomalies}, ensure_ascii=False))


if __name__ == '__main__':
    main()
