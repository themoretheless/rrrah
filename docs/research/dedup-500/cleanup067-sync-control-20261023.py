"""Whole original module, owned synthetic files only; no skill install/cleanup."""
import hashlib
import importlib.util
import json
import sys
from pathlib import Path
from tempfile import TemporaryDirectory
BASE=Path(__file__).resolve().parent
EV=BASE/'evidence/Hao0321--claude-skill-code-cleanup'
path=EV/'03-audit_core.py'
source=path.read_bytes()
pin=next(e for e in json.loads((EV/'tree.json').read_text())['tree'] if e['path']=='code-cleanup-helper/scripts/audit_core.py')
assert hashlib.sha1(b'blob '+str(len(source)).encode()+b'\0'+source).hexdigest()==pin['sha']
name='rrrah_cleanup067_native_audit'
spec=importlib.util.spec_from_file_location(name,path)
module=importlib.util.module_from_spec(spec)
sys.modules[name]=module
spec.loader.exec_module(module)
with TemporaryDirectory(prefix='rrrah-cleanup-sync-') as directory:
    base=Path(directory)
    private=base/'private'; public=base/'public'
    private.mkdir();public.mkdir()
    config={**module.DEFAULT_CONFIG,'sync':{'public_root':str(public),'normalize_text':True}}
    def sync():return module.audit_sync(private,config)
    (private/'one.txt').write_text('same\n');(public/'one.txt').write_text('same\n')
    (public/'extra.txt').write_text('public-only content\n')
    assert all(f.status=='PASS' for f in sync())
    (private/'one.txt').write_bytes(b'line\r\n');(public/'one.txt').write_bytes(b'line\n')
    assert all(f.status=='PASS' for f in sync())
    (private/'one.txt').write_bytes(b'\xff'*90);(public/'one.txt').write_bytes(b'\xfe'*90)
    assert all(f.status=='PASS' for f in sync())
    config['sync']['normalize_text']=False
    assert any(f.code=='sync-diff' for f in sync())
    config['sync']['public_root']=str(base/'absent')
    assert sync()[0].status=='NOT_CHECKED'
    (private/'bad.py').write_text('def broken(:\n')
    report=module.run_audit(private,'all')
    assert any(f['code']=='architecture-parse-error' and f['status']=='FAIL' for f in report['findings'])
    assert report['summary']['fail']>=1
    assert report['summary']['not_checked']>=1
print(json.dumps({'git_blob_verified':True,'whole_original_module_loaded':True,'controls_passed':6,'public_only_file_not_reported':True,'crlf_lf_equal_in_text_mode':True,'different_invalid_utf8_equal_in_text_mode':True,'byte_mode_detects_difference':True,'missing_sync_root_not_checked':True,'full_audit_retains_parse_failure_and_not_checked':True,'external_projects_or_upstream_suite_run':False},indent=2))
