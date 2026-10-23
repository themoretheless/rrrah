from pathlib import Path
from types import SimpleNamespace
import ast, json, os, tempfile, sys
root=Path(__file__).resolve().parents[3]
source=root/".dedup-work/mail092-20261024/mail_deduplicate/cache.py"
tree=ast.parse(source.read_text())
tree.body=[n for n in tree.body if not(isinstance(n,ast.ImportFrom) and (n.level or n.module=="platformdirs"))]
owned=Path(tempfile.mkdtemp(prefix="mail092-cache-",dir=root/".dedup-work"))
ns=dict(resolve_mail_path=lambda box,mail_id: str(Path(box._path)/mail_id),user_cache_path=lambda *a,**k:owned)
exec(compile(tree,str(source),"exec"),ns)
C,E=ns["HashCache"],ns["CacheEntry"]
boxdir=owned/"box";boxdir.mkdir();mail=boxdir/"a";mail.write_bytes(b"aaaa");box=SimpleNamespace(_path=str(boxdir));db=owned/"hashes.db"
c=C(db,"v1");assert c.lookup(box,"a") is None;c.store(box._path,"a",E("old-hash",1.0,4));assert c.commit();c.close()
st=mail.stat();mail.write_bytes(b"bbbb");os.utime(mail,ns=(st.st_atime_ns,st.st_mtime_ns));assert mail.read_bytes()==b"bbbb"
c=C(db,"v1");hit=c.lookup(box,"a");assert hit.mail_hash=="old-hash";c.close()
os.utime(mail,ns=(st.st_atime_ns,st.st_mtime_ns+1000000000));c=C(db,"v1");assert c.lookup(box,"a") is None;c.close()
c=C(db,"v2");assert c.connection.execute("SELECT count(*) FROM hashes").fetchone()[0]==0;c.close()
result=dict(scope="Original complete cache module AST with platform path/mail-path adapters only; real SQLite and physical owned files",python=sys.version,sqlite=__import__("sqlite3").sqlite_version,same_size_restored_mtime_returns_old_hash=True,changed_mtime_invalidates=True,changed_recipe_invalidates=True,owned_directory=str(owned),limits=["No full package test suite or mailbox decoding tested.","No crash/power-loss/concurrency durability qualification."])
(root/"docs/research/dedup-500/mail092-cache-result-20261024.json").write_text(json.dumps(result,indent=2)+"\n");print(json.dumps(result))
