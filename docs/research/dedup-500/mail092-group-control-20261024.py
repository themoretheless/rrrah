from pathlib import Path
from collections import Counter
from functools import cached_property
from itertools import combinations
from difflib import unified_diff
from enum import Enum
from types import SimpleNamespace
import ast, logging, json, random
root=Path(__file__).resolve().parents[3]
source=root/"docs/research/dedup-500/evidence/kdeldycke--mail-deduplicate/03-deduplicate.py"
tree=ast.parse(source.read_text()); names={"Stat","SizeDiffAboveThreshold","ContentDiffAboveThreshold","MissingTimestamps","DuplicateSet"}
body=[ast.ImportFrom(module="__future__",names=[ast.alias(name="annotations")],level=0)]+[n for n in tree.body if isinstance(n,ast.ClassDef) and n.name in names]
ns=dict(Counter=Counter,cached_property=cached_property,combinations=combinations,unified_diff=unified_diff,Enum=Enum,unique=lambda x:x,logging=logging)
exec(compile(ast.fix_missing_locations(ast.Module(body=body,type_ignores=[])),str(source),"exec"),ns)
D=ns["DuplicateSet"]
class Mail:
 def __init__(self,name,text,body_hash): self.name=name;self.body_lines=text.splitlines(keepends=True);self.size=len(text.encode());self.hash_raw_body=body_hash
 def __repr__(self): return self.name
conf=dict(size_threshold=-1,content_threshold=0,show_diff=False)
a=Mail("a","aaaa\n","same-synthetic-hash");b=Mail("b","bbbb\n","same-synthetic-hash")
s=D("synthetic-candidate",[a,b],conf);assert s.check_differences()==set()
b.hash_raw_body="other-hash"
try:D("synthetic-candidate",[a,b],conf).check_differences()
except ns["ContentDiffAboveThreshold"]: distinct_rejected=True
else:raise AssertionError("distinct body must reject")
rng=random.Random(92); asymmetric=None
for _ in range(10000):
 x="".join(rng.choice("abcd")+"\n" for _ in range(12));y="".join(rng.choice("abcd")+"\n" for _ in range(12));m=Mail("x",x,"x");n=Mail("y",y,"y");ab=s.diff(m,n);ba=s.diff(n,m)
 if ab!=ba: asymmetric=dict(a=x,b=y,forward=ab,reverse=ba);break
assert asymmetric is not None
result=dict(scope="Exact original classes via AST with synthetic mail objects; no package CLI or mailbox actions",synthetic_equal_hash_shortcuts_different_bytes=True,distinct_hash_zero_threshold_rejected=distinct_rejected,diff_direction_control=asymmetric,limits=["Injected body-hash equality is not an observed real hash collision.","Diff direction result alone does not prove a full CLI false selection.","No upstream test suite run."])
(root/"docs/research/dedup-500/mail092-group-result-20261024.json").write_text(json.dumps(result,indent=2)+"\n");print(json.dumps(result))
