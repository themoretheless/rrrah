"""Compare original exclusion preview against enabled-rule scan filtering."""
from pathlib import Path
from types import SimpleNamespace
import hashlib
import json
import re
import subprocess
import sys

R = Path(__file__).resolve().parents[1]
S = R / '.dedup-work/dupeguru1-20261019/source'
W = R / '.dedup-work/dupeguru1-exclusion-preview-20261019'


def main():
    W.mkdir(exist_ok=False)
    names = ['core/exclude.py', 'core/gui/exclude_list_dialog.py', 'core/gui/exclude_list_table.py']
    pins = {n: hashlib.sha256((S / n).read_bytes()).hexdigest() for n in names}
    for n in names:
        assert subprocess.check_output(['git', '-C', str(S), 'show', 'HEAD:' + n]) == (S / n).read_bytes()
    sys.path.insert(0, str(S))
    from core.exclude import ExcludeDict
    from core.gui.exclude_list_dialog import ExcludeListDialogCore
    from hscommon.testutil import CallLogger

    rules = ExcludeDict()
    dialog = ExcludeListDialogCore(SimpleNamespace(exclude_list=rules))
    dialog.exclude_list_table.view = CallLogger()
    pattern = r'.*\.tmp'
    dialog.add(pattern)
    assert rules.is_excluded('/synthetic', 'sample.tmp')
    enabled_preview = dialog.test_string('/synthetic/sample.tmp')
    rules.unmark(pattern)
    dialog.refresh()
    disabled_actual = rules.is_excluded('/synthetic', 'sample.tmp')
    disabled_preview = dialog.test_string('/synthetic/sample.tmp')
    highlighted = [row.regex for row in dialog.exclude_list_table.rows if row.highlight]
    assert enabled_preview and disabled_preview and not disabled_actual and highlighted == [pattern]
    before = len(rules)
    try:
        dialog.add('[')
    except re.error:
        pass
    else:
        raise AssertionError('invalid expression should be rejected')
    assert len(rules) == before
    assert not dialog.test_string('/synthetic/sample.bin')
    assert all(hashlib.sha256((S / n).read_bytes()).hexdigest() == pins[n] for n in names)
    result = {'status': 'disabled_rule_preview_matches_but_scan_filter_does_not', 'source_pins': pins,
              'enabled_preview': enabled_preview, 'disabled_preview': disabled_preview,
              'disabled_actual_exclusion': disabled_actual, 'disabled_highlighted_rules': highlighted,
              'invalid_expression_rejected_without_adding_rule': True,
              'limits': ['Actual ExcludeDict and core dialog/table logic; original CallLogger view, no Qt widget or filesystem traversal.',
                         'Preview can be useful for testing a disabled regex, but its positive highlight is not proof the path will be excluded.',
                         'No app constructor, user data, file actions, source patches or production changes.',
                         'Supplemental authored control, not an additional upstream test.']}
    (W / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({'status': result['status'], 'invalid_rejected': True}))


if __name__ == '__main__':
    main()
