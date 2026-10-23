"""Original Up2k database methods against owned, bounded temporary databases."""
import hashlib
import json
import pathlib
import sqlite3
import sys
import tempfile
from types import SimpleNamespace
root = pathlib.Path(__file__).resolve().parents[3]
checkout = root / '.dedup-work/copyparty082-20261023'
sys.path.insert(0, str(checkout))
from copyparty.up2k import Up2k, DB_VER
assert hashlib.sha256((checkout/'copyparty/up2k.py').read_bytes()).hexdigest() == '9914c93d246312e5ba962ae33ef01ff212548b5cfb3a789087dcddf6b682ae89'
u = object.__new__(Up2k)
u.timeout = 2
u.log = lambda *a, **k: None
results = []
with tempfile.TemporaryDirectory(prefix='rrrah-copyparty-db-') as td:
    folder = pathlib.Path(td)
    for legacy_index in (False, True):
        u.no_expr_idx = legacy_index
        db = folder / ('new-%s.db' % legacy_index)
        cur = u._open_db(str(db))
        assert u._read_ver(cur) == DB_VER
        assert len(cur.execute('pragma table_info(up)').fetchall()) == 8
        cur.execute('insert into up values(?,?,?,?,?,?,?,?)', ('a'*32,1,3,'','one','',0,''))
        cur.connection.commit()
        cur.connection.close()
        cur = u._open_db(str(db))
        assert cur.execute('select fn,sz from up').fetchall() == [('one',3)]
        cur.connection.close()
        results.append({'case':'create-reopen','no_expr_idx':legacy_index,'passed':True})
    db = folder/'v4.db'
    con = sqlite3.connect(db)
    con.executescript("create table up(w text, mt int, sz int, rd text, fn text); create table mt(w text,k text,v int); create table kv(k text,v int); insert into kv values('sver',4); insert into up values('original',1,3,'','one');")
    con.commit(); con.close()
    u.no_expr_idx = False
    cur = u._open_db(str(db))
    assert u._read_ver(cur) == DB_VER == 6
    assert cur.execute('select w,fn,sz from up').fetchall() == [('original','one',3)]
    cur.connection.close()
    backups = sorted(folder.glob('v4.db.bak.*'))
    versions = []
    for backup in backups:
        con = sqlite3.connect(backup)
        versions.append(u._read_ver(con.cursor()))
        assert con.execute('select w,fn,sz from up').fetchall() == [('original','one',3)]
        con.close()
    assert sorted(versions) == [4,5]
    results.append({'case':'v4-to-v6','backup_versions':versions,'passed':True})
    db = folder/'future.db'
    con = sqlite3.connect(db)
    con.executescript("create table kv(k text,v int); insert into kv values('sver',999);")
    con.commit(); con.close()
    before = db.read_bytes()
    try:
        u._open_db(str(db))
    except Exception as ex:
        assert 'only supports versions' in str(ex)
    else:
        raise AssertionError('future version accepted')
    assert db.read_bytes() == before
    assert not list(folder.glob('future.db.bak.*'))
    results.append({'case':'future-version-refusal','bytes_preserved':True,'passed':True})
print(json.dumps({'controls':results,'scope':'Original methods on bounded owned temporary SQLite files; synthetic v4 schema. No disk failure, concurrency, old SQLite engine, power-loss or server qualification.'},indent=2))
