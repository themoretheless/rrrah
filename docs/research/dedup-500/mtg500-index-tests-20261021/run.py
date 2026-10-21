from pathlib import Path
import sys,json,tempfile,sqlite3
root=Path.cwd();sys.path[:0]=[str(root/'.dedup-work/mtg500-deps-20261021'),str(root/'.dedup-work/mediahash478-pytest-20261021'),str(root/'.dedup-work/mtg500-20261021')]
import pytest
rc=pytest.main(['-q','-p','no:cacheprovider',*[str(root/'.dedup-work/mtg500-20261021/tests'/p) for p in ['test_bulk_reader.py','test_index_store.py','test_manifest_watermark.py']]])
print('ORIGINAL_PYTEST_EXIT',rc,flush=True)
from build_index import open_index,write_batch,CROP_SIGNATURE
from embed import EMBED_VERSION
with tempfile.TemporaryDirectory(prefix='mtg500-index-') as t:
 p=Path(t)/'idx.sqlite';conn=open_index(p);write_batch(conn,[('orphan',b'x',b'y')]);conn.execute('DELETE FROM art_embeddings');conn.executemany('INSERT INTO meta VALUES (?,?)',[('crop_signature',CROP_SIGNATURE),('embed_version',str(EMBED_VERSION))]);conn.commit();conn.close();conn=open_index(p)
 assert conn.execute('SELECT count(*) FROM art_hashes').fetchone()[0]==1;assert conn.execute('SELECT count(*) FROM art_embeddings').fetchone()[0]==0;assert conn.execute("SELECT value FROM meta WHERE key='algo_version'").fetchone() is None
 print('PASS open_index retains hash without embedding and absent algo_version when crop/embed metadata match')
 assert len(conn.execute('SELECT hash FROM art_hashes').fetchone()[0])==1
 print('PASS write_batch accepts wrong-length hash/embedding blobs')
 conn.execute("CREATE TRIGGER reject_embedding BEFORE INSERT ON art_embeddings BEGIN SELECT RAISE(ABORT,'fixture failure'); END")
 try:write_batch(conn,[('partial',bytes(128),bytes(384))])
 except sqlite3.IntegrityError:pass
 else:raise AssertionError('expected injected failure')
 assert conn.in_transaction;assert conn.execute("SELECT count(*) FROM art_hashes WHERE illustration_id='partial'").fetchone()[0]==1;assert conn.execute("SELECT count(*) FROM art_embeddings WHERE illustration_id='partial'").fetchone()[0]==0
 conn.rollback();conn.close();print('PASS failed write_batch leaves partial pending transaction; authored caller rolls back,not committed')
print(json.dumps({'new_authored_controls':3,'original_test_exit':int(rc)}));sys.exit(int(rc))
