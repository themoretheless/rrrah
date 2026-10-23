"""Pinned SQL/schema control; does not execute Android Room or app code."""
import hashlib
import json
import re
import sqlite3
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
CACHE = ROOT / '.dedup-work/cleansweep048-20261023'
TREE = ROOT / 'docs/research/dedup-500/evidence/loopotto--CleanSweep/tree.json'
pins = {e['path']: e['sha'] for e in json.loads(TREE.read_text())['tree'] if e['type'] == 'blob'}

def read(path):
    data = (CACHE / path).read_bytes()
    assert hashlib.sha1(b'blob ' + str(len(data)).encode() + b'\0' + data).hexdigest() == pins[path]
    return data.decode()

schemas = {v: json.loads(read(f'app/schemas/com.cleansweep.data.db.CleanSweepDatabase/{v}.json'))['database'] for v in (1, 2, 3, 5)}
source = read('app/src/main/java/com/cleansweep/data/db/CleanSweepDatabase.kt')
provider = read('app/src/main/java/com/cleansweep/di/DatabaseModule.kt')
assert 'version = 3' in source
assert 'CleanSweepDatabase.MIGRATION_1_2' in provider and 'CleanSweepDatabase.MIGRATION_2_3' in provider
sql = re.findall(r'db\.execSQL\((?:"""(.*?)"""\.trimIndent\(\)|"([^"\n]*)")\)', source, re.S)
statements = [(a or b).strip() for a, b in sql]
assert len(statements) == 3

def create(version):
    db = sqlite3.connect(':memory:')
    db.execute('PRAGMA foreign_keys=ON')
    for e in schemas[version]['entities']:
        db.execute(e['createSql'].replace('${TABLE_NAME}', e['tableName']))
        for i in e.get('indices', []):
            db.execute(i['createSql'].replace('${TABLE_NAME}', e['tableName']))
    return db

def signature(db, table):
    return {
        'columns': db.execute(f'PRAGMA table_info("{table}")').fetchall(),
        'foreign_keys': db.execute(f'PRAGMA foreign_key_list("{table}")').fetchall(),
        'indexes': db.execute(f'PRAGMA index_list("{table}")').fetchall(),
    }

db = create(1)
db.execute("INSERT INTO scan_result_groups VALUES ('g','DUPLICATE','[]',NULL,'sig',1)")
db.execute("INSERT INTO file_signature_cache VALUES ('/a',1,4,'hash')")
db.execute("INSERT INTO phash_cache VALUES ('/a',1,4,'p',NULL)")
db.execute("INSERT INTO similar_group_cache VALUES ('old-group','/a')")
db.execute(statements[0])
expected2 = create(2)
assert all(signature(db,e['tableName']) == signature(expected2,e['tableName']) for e in schemas[2]['entities'])
assert db.execute('SELECT scopeType FROM scan_result_groups').fetchall() == [('FULL',)]
for statement in statements[1:]:
    db.execute(statement)
expected3 = create(3)
assert all(signature(db,e['tableName']) == signature(expected3,e['tableName']) for e in schemas[3]['entities'])
assert db.execute('SELECT * FROM file_signature_cache').fetchall() == [('/a',1,4,'hash')]
assert db.execute('SELECT * FROM similar_group_cache').fetchall() == [('old-group','/a')]
expected_names = {e['tableName'] for e in schemas[3]['entities']}
actual_names = {r[0] for r in db.execute("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'")}
assert actual_names - expected_names == {'similar_group_cache'}
assert db.execute('PRAGMA foreign_key_check').fetchall() == []
print(json.dumps({'sqlite_version':sqlite3.sqlite_version,'pinned_inputs_verified':6,'migration_statements':statements,'v1_to_v2_expected_tables_match':True,'v2_to_v3_expected_tables_match':True,'signature_and_scan_group_retained':True,'scope_default':'FULL','extra_table_after_v3':sorted(actual_names-expected_names),'stale_group_row_retained':True,'foreign_key_violations':0,'android_room_runtime_run':False},indent=2))
