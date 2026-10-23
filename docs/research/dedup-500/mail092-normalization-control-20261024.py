from pathlib import Path
from enum import StrEnum
from types import SimpleNamespace
import ast,json
root=Path(__file__).resolve().parents[3];src=root/".dedup-work/mail092-20261024/mail_deduplicate"
def load(name,extra):
 tree=ast.parse((src/name).read_text());tree.body=[n for n in tree.body if not(isinstance(n,ast.ImportFrom) and (n.level or n.module=="click_extra"))];ns=dict(StrEnum=StrEnum,**extra);exec(compile(tree,str(src/name),"exec"),ns);return ns
ns=load("mail.py",{});M=ns["DedupMailMixin"];T=ns["TimeSource"]
def mail(body,date="Wed, 01 Jan 2020 01:00:00 +0000",subject="Topic"):
 raw=f"From: x@example.com\nTo: y@example.com\nSubject: {subject}\nDate: {date}\nContent-Type: text/plain; charset=utf-8\n\n{body}";m=M(raw.encode("utf-8"));m.conf=dict(hash_headers=("from","to","subject","date","content-type"),minimal_headers=5,time_source=T.DATE_HEADER);return m
x=mail("one",subject="Topic");y=mail("two",date="Wed, 01 Jan 2020 22:00:00 +0000",subject="Re: Topic");assert x.hash_key()==y.hash_key();assert x.hash_raw_body!=y.hash_raw_body
assert mail("ab c").hash_normalized_body==mail("a bc").hash_normalized_body
assert mail("a\nb\n").hash_raw_body==mail("a\r\nb").hash_raw_body
u=mail("яя");assert u.size==2
old=mail("aa");fresh=mail("abcdefghij",date="Tue, 01 Jan 2030 00:00:00 +0000");_=old.body_lines;old_date=old.timestamp;old_size=old.size
class Box:
 _path="synthetic-owned-box"
 def __getitem__(self,key):return fresh
old.add_box_metadata(Box(),"a");old.dehydrate();old.hydrate();assert old.body_lines==["abcdefghij"];assert old.size==old_size==2;assert old.timestamp==old_date;assert old.timestamp!=fresh.timestamp
strategy=load("strategy.py",{});aliases=[(s.name,s.function.__name__) for s in strategy["Strategy"]];assert len(aliases)==24
result=dict(scope="Original full mail/strategy modules via AST, optional presentation imports omitted; real stdlib Message parser, synthetic in-memory box only",header_equivalence_despite_different_body_date_hour_subject_prefix=True,normalized_whitespace_boundary_collision=True,raw_body_line_ending_equivalence=True,unicode_size_characters=2,unicode_body_utf8_bytes=4,rehydrated_body_characters=10,retained_cached_size=old.size,retained_cached_date_after_rehydrate=True,strategy_members_resolve=24,limits=["These normalizations are mail-domain policy, not cryptographic collisions.","Synthetic source replacement between hydrate stages, not a measured live mailbox race.","No full upstream test suite or CLI run."])
(root/"docs/research/dedup-500/mail092-normalization-result-20261024.json").write_text(json.dumps(result,indent=2)+"\n");print(json.dumps(result))
