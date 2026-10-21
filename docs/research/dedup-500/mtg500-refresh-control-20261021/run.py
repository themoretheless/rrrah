from pathlib import Path
import sys,os,json,tempfile,sqlite3
from unittest.mock import patch
root=Path.cwd();sys.path[:0]=[str(root/'.dedup-work/mtg500-deps-20261021'),str(root/'.dedup-work/mtg500-20261021')]
import build_index as b
from embed import EMBED_VERSION
from image_hash import HASH_ALGO_VERSION
class NoNetwork:
 def __init__(self,*a,**kw):pass
 def __enter__(self):return self
 def __exit__(self,*a):return False
with tempfile.TemporaryDirectory(prefix='mtg500-refresh-') as temp:
 t=Path(temp);db=t/'idx.sqlite';conn=b.open_index(db);b.write_batch(conn,[(str(i),bytes(128),bytes(384)) for i in range(1000)])
 stamp='2026-10-21T00:00:00Z';retry={str(i):'https://invalid.example/fixture' for i in range(500)}
 conn.executemany('INSERT INTO meta VALUES (?,?)',[('algo_version',str(HASH_ALGO_VERSION)),('embed_version',str(EMBED_VERSION)),('crop_signature',b.CROP_SIGNATURE),('bulk_updated_at',stamp),('image_updated_through',stamp),('retry_images',json.dumps(retry))]);conn.commit();conn.close()
 produced=[]
 def process(todo,on_result):
  assert todo==retry
  for key in todo:on_result((key,b'\x01'*128,b'\x02'*384));produced.append(key)
  return len(todo),{}
 previous=Path.cwd()
 try:
  os.chdir(t)
  with patch.object(b,'OUT',db),patch.object(b,'bulk_entry',return_value={'updated_at':stamp}),patch.object(b.httpx,'Client',NoNetwork),patch.object(b,'updated_card_ids',return_value=([],stamp,None)),patch.object(b,'hash_images',side_effect=process),patch.object(sys,'argv',['build_index.py']):
   try:b.main()
   except SystemExit as e:assert e.code==1
   else:raise AssertionError('expected false row-floor abort')
 finally:os.chdir(previous)
 conn=sqlite3.connect(db);count=conn.execute('SELECT count(*) FROM art_hashes').fetchone()[0];updated=conn.execute('SELECT count(*) FROM art_hashes WHERE hash=?',(b'\x01'*128,)).fetchone()[0];conn.close();assert(count,updated,len(produced))==(1000,500,500)
 print(json.dumps({'control':'refresh double-count false abort','existing_groups':1000,'refreshed_existing_groups':500,'failed_images':0,'result_rows':count,'updated_rows':updated,'computed_expected':1500,'actual_distinct_expected':1000,'exit_code':1,'network_and_inference':'authored stubs;actual original main and SQLite'}))
