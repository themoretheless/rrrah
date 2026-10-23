from pathlib import Path
import sys,json
root=Path(__file__).resolve().parents[3];sys.path.insert(0,str(root/".dedup-work/mail092-20261024"))
from mail_deduplicate import deduplicate as module
D=module.Deduplicate;d=object.__new__(D);original=module.ProcessPoolExecutor;initialized=[];closed=[]
def init():initialized.append(True)
def refused(**kwargs):raise OSError("synthetic constructor refusal")
module.ProcessPoolExecutor=refused
try:
 with d.worker_pool(2,init,(),"Hash","hashing") as pool:
  assert pool is None;assert list(d.fan_out(pool,lambda n:n*2,[("a",1),("b",2)],2,1))==[("a",2),("b",4)]
 assert initialized==[True]
 class DelayedFailure:
  def __init__(self,**kwargs):pass
  def map(self,*args,**kwargs):raise OSError("synthetic delayed process startup refusal")
  def shutdown(self):closed.append(True)
 module.ProcessPoolExecutor=DelayedFailure
 try:
  with d.worker_pool(2,init,(),"Hash","hashing") as pool:
   list(d.fan_out(pool,lambda n:n,[("a",1)],2,1))
 except OSError as error:assert "delayed" in str(error)
 else:raise AssertionError("delayed map refusal must propagate")
 assert closed==[True];assert len(initialized)==1
 consumed=[]
 def tasks():
  for i in range(5):consumed.append(i);yield i,i
 stream=d.fan_out(None,lambda n:n, tasks(),2,1);assert next(stream)==(0,0);assert consumed==[0,1];assert list(stream)==[(1,1),(2,2),(3,3),(4,4)]
finally:module.ProcessPoolExecutor=original
result=dict(scope="Normally imported original worker_pool and fan_out; synthetic executor constructor/map errors, no processes spawned",constructor_refusal_falls_back=True,delayed_map_refusal_propagates=True,executor_shutdown_on_delayed_failure=True,first_result_consumes_two_task_window=True,limits=["Synthetic executor errors, not measured OS startup failure.","No throughput, RSS or universal worker recovery qualification."])
(root/"docs/research/dedup-500/mail092-pool-result-20261024.json").write_text(json.dumps(result,indent=2)+"\n");print(json.dumps(result))
