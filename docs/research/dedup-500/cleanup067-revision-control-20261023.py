"""Original revision checker on owned roots and deterministic callback controls."""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
from tempfile import TemporaryDirectory
BASE=Path(__file__).resolve().parent
EV=BASE/'evidence/Hao0321--claude-skill-code-cleanup'
p=BASE.parents[2]/'.dedup-work/cleanup067-20261023/code-cleanup-helper/scripts/check_skill_revision.py'
data=p.read_bytes();pin=next(x for x in json.loads((EV/'tree.json').read_text())['tree'] if x['path']=='code-cleanup-helper/scripts/check_skill_revision.py')
assert hashlib.sha1(b'blob '+str(len(data)).encode()+b'\0'+data).hexdigest()==pin['sha']
spec=importlib.util.spec_from_file_location('rrrah_cleanup067_revision',p);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
with TemporaryDirectory(prefix='rrrah-cleanup-revision-') as directory:
    root=Path(directory);(root/'SKILL.md').write_text('authored fixture\n');(root/'one.txt').write_text('one\n')
    document=m.capture_document([root]);assert m.verify_document(document)['status']=='CURRENT'
    bad=copy.deepcopy(document);bad['snapshot']['roots']=2
    try:m.validate_evidence(bad)
    except m.RevisionError:pass
    else:raise AssertionError('inconsistent aggregate accepted')
    (root/'alias.txt').symlink_to(root/'one.txt')
    try:m.capture_document([root])
    except m.RevisionError:pass
    else:raise AssertionError('symlink accepted')
    (root/'alias.txt').unlink()
    try:m.scan_roots([root,root/'.'])
    except m.RevisionError:pass
    else:raise AssertionError('duplicate root accepted')
    calls=[]
    def scanner(_roots):
        calls.append(1)
        raise m.RevisionUnstable('authored read-time mutation signal')
    try:m.capture_consistent([root],max_attempts=3,scanner=scanner)
    except m.RevisionUnstable:pass
    else:raise AssertionError('unstable read accepted')
    assert len(calls)==1
    assert m._valid_relative('./one.txt')=='./one.txt'
print(json.dumps({'git_blob_verified':True,'additional_controls_passed':6,'unchanged_current':True,'inconsistent_root_count_rejected':True,'nonignored_symlink_rejected':True,'canonical_duplicate_roots_rejected':True,'read_time_unstable_aborts_first_attempt':True,'dot_path_alias_not_canonicalized':True,'installed_or_external_skill_touched':False},indent=2))
