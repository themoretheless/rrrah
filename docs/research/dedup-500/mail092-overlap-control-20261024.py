from pathlib import Path
from types import SimpleNamespace
import sys,json,tempfile
root=Path(__file__).resolve().parents[3];sys.path.insert(0,str(root/".dedup-work/mail092-20261024"))
from mail_deduplicate.mail_box import BoxFormat
from mail_deduplicate.deduplicate import Deduplicate,Stat
owned=Path(tempfile.mkdtemp(prefix="mail092-overlap-",dir=root/".dedup-work"));parent=owned/"parent";child=parent/"Sub"
for p in (parent,child):
 box=BoxFormat.MAILDIR.constructor(p,create=True);box.add(b"Subject: synthetic\n\nbody");box.close()
conf=dict(action=SimpleNamespace(verb="copy"),cache=False,input_format=BoxFormat.MAILDIR,force_unlock=False)
a=Deduplicate(conf);a.add_source(child);a.add_source(parent);observed=sum(len(box) for box in a.sources.values());count=a.stats[Stat.MAIL_FOUND];assert observed==2 and count==3;a.close_all()
b=Deduplicate(conf);b.add_source(parent)
try:b.add_source(child)
except ValueError:refused=True
else:raise AssertionError("explicit duplicate child should reject")
b.close_all()
result=dict(scope="Normally imported original Deduplicate.add_source; physical owned parent/child Maildirs; no hashing or mail action",child_then_parent_found_count=count,child_then_parent_current_messages=observed,parent_then_child_refused=refused,owned_directory=str(owned),limits=["Registration/accounting only, not full CLI final-state reproduction.","No production changes or real mailbox touched."])
(root/"docs/research/dedup-500/mail092-overlap-result-20261024.json").write_text(json.dumps(result,indent=2)+"\n");print(json.dumps(result))
