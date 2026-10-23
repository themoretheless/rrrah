from pathlib import Path
from itertools import combinations_with_replacement
import sys,json
root=Path(__file__).resolve().parents[3];sys.path.insert(0,str(root/".dedup-work/mail092-20261024"))
from mail_deduplicate.deduplicate import DuplicateSet,SizeDiffAboveThreshold
class Mail:
 def __init__(self,i,size):self.i=i;self.size=size;self.hash_raw_body="synthetic-shared-body"
 def __repr__(self):return f"mail{self.i}"
count=0;suboptimal=[];errors=0
for sizes in combinations_with_replacement(range(8),6):
 for threshold in range(3):
  count+=1;mails=[Mail(i,size) for i,size in enumerate(sizes)];s=DuplicateSet("synthetic",mails,dict(size_threshold=threshold,content_threshold=-1,show_diff=False))
  optimum=max(sum(lo<=v<=lo+threshold for v in sizes) for lo in sizes)
  try:evicted=s.check_differences()
  except SizeDiffAboveThreshold:
   assert optimum<2;errors+=1;continue
  remaining=set(mails)-evicted;values=[m.size for m in remaining];assert max(values)-min(values)<=threshold
  if len(remaining)<optimum and len(suboptimal)<3:suboptimal.append(dict(sizes=sizes,threshold=threshold,greedy_kept=sorted(values),maximum_size=optimum))
result=dict(scope="Original imported DuplicateSet.check_differences; synthetic size-only mails, content check disabled",cases=count,coherence_violations=0,sets_rejected_without_two_member_core=errors,suboptimal_examples=suboptimal,limits=["Synthetic size-only matrix, not content-diff or full CLI qualification.","No maximum-cardinality guarantee inferred beyond enumerated cases."])
(root/"docs/research/dedup-500/mail092-size-graph-result-20261024.json").write_text(json.dumps(result,indent=2)+"\n");print(json.dumps(result))
