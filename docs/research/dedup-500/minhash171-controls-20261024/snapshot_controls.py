"""Original ReaderPlan/StageCheckpoints, actual local fsspec and owned files.

Tests metadata binding only, not stale-filter deletion or distributed recovery.
"""
import hashlib
import json
import os
from pathlib import Path
import sys
import tempfile

sys.path.insert(0,str(Path(__file__).with_name('runtime')))
from minhash_dedup.partition import ReaderPlan
from minhash_dedup.checkpoints import StageCheckpoints
root=Path(tempfile.mkdtemp(prefix='minhash171-snapshot-owned-'))
input_dir=root/'input';input_dir.mkdir();source=input_dir/'owned.jsonl'
original=b'{"text":"alpha"}\n';changed=b'{"text":"bravo"}\n';assert len(original)==len(changed)
source.write_bytes(original)
signatures=root/'signatures'
plan=ReaderPlan.prepare(signatures,[input_dir],1,1,{'owned':1})
assert plan.shard(0)==(0,['owned.jsonl'])
source.write_bytes(original+b' ')
size_error=None
try:plan.shard(0)
except ValueError as exc:size_error=str(exc)
assert size_error is not None
source.write_bytes(changed)
assert plan.shard(0)==(0,['owned.jsonl'])
checkpoint_root=root/'checkpoints'
checks=StageCheckpoints(checkpoint_root,{'owned':1})
checks.begin('2',expected_files=['buckets/000_00.dups'])
artifact=checkpoint_root/'buckets/000_00.dups';artifact.parent.mkdir(parents=True);artifact.write_bytes(b'AAAA')
checks.complete('2');generation=checks.require_complete('2');stamp=artifact.stat()
artifact.write_bytes(b'BBBB');os.utime(artifact,ns=(stamp.st_atime_ns,stamp.st_mtime_ns))
assert checks.require_complete('2')==generation
payload={'scope':__doc__,'owned_fixture_root':str(root),'authored_controls_run':4,'original_tests_run':0,'observations':['unchanged reader-plan shard accepted','different-size input rejected',{'case':'same-size changed input content accepted by ReaderPlan.shard','original_sha256':hashlib.sha256(original).hexdigest(),'changed_sha256':hashlib.sha256(changed).hexdigest()},{'case':'changed output with same size and restored mtime accepted by StageCheckpoints','generation_unchanged':True,'qualification':'metadata-only inventory is documented; no actual corrupted stage/real corpus execution'}]}
Path(__file__).with_name('snapshot-results.json').write_text(json.dumps(payload,indent=2)+'\n');print(json.dumps(payload,indent=2))
