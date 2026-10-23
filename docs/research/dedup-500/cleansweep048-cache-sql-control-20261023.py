"""SQLite shape control, not execution of Android Room-generated code."""
import sqlite3,json
con=sqlite3.connect(':memory:');con.execute('PRAGMA foreign_keys=ON')
con.executescript('''CREATE TABLE scan_result_groups(uniqueId TEXT PRIMARY KEY NOT NULL,groupType TEXT NOT NULL,scopeType TEXT NOT NULL,timestamp INTEGER NOT NULL);
CREATE TABLE media_item_refs(id INTEGER PRIMARY KEY AUTOINCREMENT,groupId TEXT NOT NULL,mediaItemId TEXT NOT NULL,FOREIGN KEY(groupId) REFERENCES scan_result_groups(uniqueId) ON DELETE CASCADE);''')
con.execute('INSERT OR REPLACE INTO scan_result_groups VALUES(?,?,?,?)',('UNSCANNABLE_SUMMARY','SUMMARY_UNSCANNABLE','FULL',1))
assert con.execute("SELECT * FROM scan_result_groups WHERE groupType = 'UNSCANNABLE_SUMMARY' AND scopeType = ? LIMIT 1",('FULL',)).fetchall()==[]
assert len(con.execute("SELECT * FROM scan_result_groups WHERE groupType != 'UNSCANNABLE_SUMMARY' AND scopeType = ? ORDER BY timestamp DESC",('FULL',)).fetchall())==1
con.execute('INSERT OR REPLACE INTO scan_result_groups VALUES(?,?,?,?)',('same-signature','EXACT','FULL',2))
con.executemany('INSERT INTO media_item_refs(groupId,mediaItemId) VALUES(?,?)',[('same-signature','a'),('same-signature','b')])
assert con.execute('SELECT count(*) FROM media_item_refs').fetchone()[0]==2
con.execute('INSERT OR REPLACE INTO scan_result_groups VALUES(?,?,?,?)',('same-signature','EXACT','SCOPED',3))
assert con.execute('SELECT count(*) FROM media_item_refs').fetchone()[0]==0
assert con.execute("SELECT count(*) FROM scan_result_groups WHERE scopeType='FULL' AND groupType='EXACT'").fetchone()[0]==0
con.execute('INSERT OR REPLACE INTO scan_result_groups VALUES(?,?,?,?)',('UNSCANNABLE_SUMMARY','SUMMARY_UNSCANNABLE','SCOPED',4))
assert con.execute("SELECT count(*) FROM scan_result_groups WHERE scopeType='FULL'").fetchone()[0]==0
print(json.dumps({'sqlite_version':sqlite3.sqlite_version,'checks_passed':6,'summary_reader_misses_written_type':True,'scoped_replace_deletes_full_group_and_refs':True,'summary_key_shared_across_scopes':True}))
