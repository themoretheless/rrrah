"""Finite original Up2k spool controls; no server or upstream test allocator."""
import gzip
import hashlib
import json
import pathlib
import sqlite3
import sys
import tempfile

root = pathlib.Path(__file__).resolve().parents[3]
checkout = root / '.dedup-work/copyparty082-20261023'
sys.path.insert(0, str(checkout))
from copyparty.up2k import Up2k

source = checkout / 'copyparty/up2k.py'
assert hashlib.sha256(source.read_bytes()).hexdigest() == '9914c93d246312e5ba962ae33ef01ff212548b5cfb3a789087dcddf6b682ae89'
u = object.__new__(Up2k)
u.stop = False
u.spools = set()
u.log = lambda *a, **k: None
con = sqlite3.connect(':memory:')
con.executescript('create table up(w text, rd text, fn text); create table mt(w text,k text,v text);')
rows = [('a'*32, 'dir\nline', 'file\nname'), ('b'*32, '', 'ordinary')]
con.executemany('insert into up values(?,?,?)', rows)
tf, count = u._spool_warks(con.cursor(), 'select w,rd,fn from up order by w', (), 0)
assert count == 2 and tf in u.spools
with gzip.GzipFile(fileobj=tf, mode='rb') as gf:
    recovered = [tuple(z[:-1].decode().replace('\x00\x02', '\n').split('\x00\x01')) for z in gf]
assert recovered == rows
u._unspool(tf)
assert tf.closed and not u.spools
con.executemany('insert into mt values(?,?,?)', [('a'*16,'t:mtp','a'), ('b'*16,'artist','b')])
tf, count = u._spool_warks(con.cursor(), 'select w from up order by w', (), 1)
with gzip.GzipFile(fileobj=tf, mode='rb') as gf:
    pending = gf.read().decode().splitlines()
assert count == 1 and pending == ['a'*32]
u._unspool(tf)
u.stop = True
tf, count = u._spool_warks(con.cursor(), 'select w from up', (), 0)
assert count == -1 and tf not in u.spools and not tf.closed
# Caller owns this returned cancellation handle; explicitly release it here.
tf.close()
con.close()
print(json.dumps({'source_sha256':hashlib.sha256(source.read_bytes()).hexdigest(), 'controls':3, 'newline_roundtrip':True, 'pending_marker_filter':True, 'prelatched_cancel_count':count, 'cancelled_handle_registered':False, 'cancelled_handle_explicitly_closed':tf.closed, 'scope':'Original methods, in-memory SQLite, two rows; no background lifecycle, disk rollover, server or performance qualification.'}, indent=2))
