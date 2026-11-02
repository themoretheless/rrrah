import ast,sqlite3,json
from pathlib import Path
class OwnedSqliteTable:
 def __init__(self):self.db=sqlite3.connect(':memory:');self.db.row_factory=sqlite3.Row
 def getName(self):return self._table_name
 def getCursor(self,*args):return self.db.cursor()
 def createIndexIfNotExists(self,name,columns,unique):self.db.execute(f'CREATE {"UNIQUE" if unique else ""} INDEX IF NOT EXISTS "{name}" ON "{self.getName()}" ({",".join(columns)})')
 def startTimer(self):pass
 def stopTimer(self,*args):pass
b=Path('docs/research/dedup-500/evidence/sergey-dryabzhinsky--dedupsqlfs');t=ast.parse((b/'08-hash.py').read_text());cls=next(n for n in t.body if isinstance(n,ast.ClassDef));ns={'Table':OwnedSqliteTable,'Binary':sqlite3.Binary};exec(compile(ast.Module(body=[cls],type_ignores=[]),'original-TableHash','exec'),ns);table=ns['TableHash']();table.create();a=table.insert(b'owned-hash-a');second=table.insert(b'owned-hash-b');assert table.find(b'owned-hash-a')==a and table.get(a)==b'owned-hash-a'
try:table.insert(b'owned-hash-a');raise AssertionError('duplicate digest admitted')
except sqlite3.IntegrityError:pass
try:table.update(a,b'owned-hash-b');raise AssertionError('conflicting update admitted')
except sqlite3.IntegrityError:pass
assert table.get(a)==b'owned-hash-a' and table.get(second)==b'owned-hash-b';assert table.get_hash_ids(a,second)=={a}
out={'controls_passed':4,'scope':'Original TableHash AST class;actual stdlib SQLite in-memory DB;explicit base-table timing/connection/index adapter;no FUSE,compression,filesystem,manager or rehash execution','findings':['Insert/find/get positive.','Unique digest index rejects repeated digest insertion.','In-place update to digest currently owned by another row raises IntegrityError;original values retained.','Hash ID interval excludes upper bound.'],'rehash_source_only':['Rehash updates digest rows in-place with batch commits;permutation/conflict and midmigration failure behavior not end-to-end qualified.','Runtime hash_function changed before migration succeeds;restoration on exception absent in reviewed function.']};Path(__file__).with_name('results.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out))
