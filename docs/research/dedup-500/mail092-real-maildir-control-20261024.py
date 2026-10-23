from pathlib import Path
import json,tempfile,sys
root=Path(__file__).resolve().parents[3];sys.path.insert(0,str(root/".dedup-work/mail092-20261024"))
from mail_deduplicate.mail_box import BoxFormat
from mail_deduplicate.mail import TimeSource
owned=Path(tempfile.mkdtemp(prefix="mail092-real-maildir-",dir=root/".dedup-work"))
def raw(body,date):return f"Subject: synthetic\nDate: {date}\nContent-Type: text/plain; charset=utf-8\n\n{body}".encode()
box=BoxFormat.MAILDIR.constructor(owned/"box",create=True)
key=box.add(raw("aa","Wed, 01 Jan 2020 00:00:00 +0000"));old=box[key];old.add_box_metadata(box,key);old.conf=dict(time_source=TimeSource.DATE_HEADER)
assert old.body_lines==["aa"];old_date=old.timestamp;assert old.size==2;old.dehydrate()
box[key]=raw("abcdefghij","Tue, 01 Jan 2030 00:00:00 +0000");fresh=box[key];fresh.conf=old.conf
assert fresh.timestamp!=old_date and fresh.size==10
old.hydrate();assert old.body_lines==["abcdefghij"];assert old.size==2 and old.timestamp==old_date
result=dict(scope="Unmodified package imported normally; original BoxFormat Maildir constructor and physical owned message replacement; no AST adapters",old_cached_size=old.size,new_body_characters=len("".join(old.body_lines)),fresh_size=fresh.size,old_cached_timestamp=old.timestamp,fresh_timestamp=fresh.timestamp,owned_directory=str(owned))
box.close();(root/"docs/research/dedup-500/mail092-real-maildir-result-20261024.json").write_text(json.dumps(result,indent=2)+"\n");print(json.dumps(result))
