#!/usr/bin/env python3
"""Record the three remaining fully read pages; retain outstanding asset/lock scope."""
import hashlib
import json
from pathlib import Path

R = Path(__file__).resolve().parents[1]
D = R / 'docs/research/dedup-500'
S = Path('/Users/themoretheless/.codex/tmp/rrrah-skiff-source-20261012')

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

parent = D / 'skiff-full-file-prefix-114-space-page-review-20261012.json'
p = json.loads(parent.read_text())
assert all(sha(f) == value for f, value in p['pins'].items())
module = D / 'skiff-exact-module-source-runtime-review-20261012.json'
m = json.loads(module.read_text())
new = ['src/pages/ApplicationCleanupPage.tsx', 'src/pages/EnvironmentPage.tsx',
       'src/pages/OverviewPage.tsx']
read = sorted(set(p['full_files_read'] + new))
assert len(read) == 117
assert all(sha(S / n) == m['archive_inventory'][n]['sha256'] for n in read)
out = D / 'skiff-full-file-prefix-117-final-pages-review-20261012.json'
assert not out.exists()
out.write_text(json.dumps(dict(
    status='verified_complete_remaining_page_reads_partial_repository',
    repository=m['repository'], commit=m['commit'], fully_read_file_count=117,
    total_files=129, full_files_read=read, new_full_files_read=new,
    remaining_files_not_fully_reviewed=sorted(set(m['archive_inventory']) - set(read)),
    findings=[
        'ApplicationCleanupPage requests uninstall only from explicit confirmation controls, submitting selected IDs. Busy state disables action controls. Cancel dismisses confirmation; it does not abort an active backend operation. No real package command or mounted UI executed.',
        'Application icon effect uses a local cancelled flag to ignore stale responses; no backend cancellation is submitted. Optional icon errors retain fallback. Selection is cleared after rescans and successful uninstall response handling.',
        'EnvironmentPage preserves original key/value in submitted changes and validates changed entries before showing confirmation. Earlier backend runtime evidence shows those originals do not prevent stale line overwrite. UI forwarding is not backend source identity validation.',
        'Environment duplicate-variable comparison lowercases keys within its write scope, including Unix shells. This is a source-level conservative validation rule, not a demonstrated filesystem or shell behavior. PATH entry values use trimmed equality.',
        'Environment save confirmation shows dirty targets and invokes backend with source/path/line/original fields. Rescan asks before discarding unsaved edits. No active-operation cancellation token or source digest is submitted. No real environment inventory/config/registry was accessed.',
        'OverviewPage renders parent-provided target, disk and run metrics; category sizes are summed from cleanable targets. It adds no overlap/physical identity accounting or duplicate matcher. Its cleanup action opens the cleanup view; scan delegates to parent callback.',
    ], implementation_adopted=False, new_runtime_tests=0,
    pins={str(f): sha(f) for f in [parent, module, Path(__file__)] + [S / n for n in read]},
    scope='All three pages fully read. 117/129 archive files reviewed; two locks, four translations and six binary icons remain. Source confirmation/cancellation observations only, no mounted React/Tauri, real filesystem mutation, uninstall, environment access, resource/platform/full500 completion.',
), indent=2) + '\n')
print('117/129 files fully reviewed; 12 remaining')
