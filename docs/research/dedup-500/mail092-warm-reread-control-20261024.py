from pathlib import Path
from mailbox import Maildir
from click.testing import CliRunner
import sys,tempfile,json
root=Path(__file__).resolve().parents[3];sys.path.insert(0,str(root/".dedup-work/mail092-20261024"))
from mail_deduplicate.cli import mdedup
from mail_deduplicate.deduplicate import Deduplicate
owned=Path(tempfile.mkdtemp(prefix="mail092-warm-reread-",dir=root/".dedup-work"));boxpath=owned/"box";box=Maildir(boxpath,create=True)
raw=b"Date: Mon, 04 Mar 2024 10:00:00 +0000\nFrom: x@example.com\nTo: y@example.com\nSubject: synthetic\nMessage-ID: <same@example.com>\n\nbody\n"
for _ in range(2):box.add(raw)
box.close();args=["--no-config","--color=never","--jobs=1","--hash-body=raw",f"--cache-path={owned / 'cache.db'}","--strategy=select-one","--action=delete-discarded","--dry-run",str(boxpath)]
runner=CliRunner();seed=runner.invoke(mdedup,args);assert seed.exit_code==0,seed.exception
original_get=Maildir.get_file;original_build=Deduplicate.build_sets;phase=[];counts={}
def get_file(self,key):counts["selection" if phase else "hashing"]=counts.get("selection" if phase else "hashing",0)+1;return original_get(self,key)
def build(self):
 phase.append(True)
 try:return original_build(self)
 finally:phase.pop()
Maildir.get_file=get_file;Deduplicate.build_sets=build
try:
 warm=runner.invoke(mdedup,["--content-threshold=-1",*args]);assert warm.exit_code==0,warm.exception;first=dict(counts);assert first.get("hashing",0)==0 and first.get("selection",0)==2
 counts.clear();unchecked=runner.invoke(mdedup,["--content-threshold=-1","--size-threshold=-1",*args]);assert unchecked.exit_code==0,unchecked.exception;second=dict(counts);assert sum(second.values())==0
finally:Maildir.get_file=original_get;Deduplicate.build_sets=original_build
result=dict(scope="Normally imported CLI, two physical owned Maildir copies, raw body and warmed owned SQLite cache; read-call spies only, dry-run",content_check_disabled_reads=first,both_checks_disabled_reads=second,limits=["Two-message deterministic read count; no throughput/RSS benchmark.","No source modification, actual deletion or user cache access."])
(root/"docs/research/dedup-500/mail092-warm-reread-result-20261024.json").write_text(json.dumps(result,indent=2)+"\n");print(json.dumps(result))
