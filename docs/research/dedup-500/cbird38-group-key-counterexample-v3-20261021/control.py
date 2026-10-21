"""Synthetic preimage counterexample, not execution of Qt or Database::filterMatches."""
from pathlib import Path
import sys,json
source=(Path(sys.argv[1])/'src/database.cpp').read_text()
assert 'for (const Media& m : copy) str += m.path();' in source
assert 'uint hash = qHash(str);' in source
left=sorted(['/root/a','/root/b/root/c'])
right=sorted(['/root/a/root/b','/root/c'])
assert left!=right
assert ''.join(left)==''.join(right)
print(json.dumps({'name':'different-two-member-path-groups-have-identical-hash-input','passed':True,'left':left,'right':right,'identical_preimage':''.join(left),'scope':'ASCII string-concatenation counterexample; no Qt/hash/database execution'}))
