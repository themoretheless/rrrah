from pathlib import Path
import re,json,sqlite3
s=Path('.dedup-work/imagehash472-20261021/db.cpp').read_text();start=s.index('"WITH image_query AS ("');end=s.index('\n\t\t];',start)
query=''.join(json.loads(x) for x in re.findall(r'"(?:[^"\\]|\\.)*"',s[start:end]))
db=sqlite3.connect(':memory:');db.executescript('CREATE TABLE images(id INTEGER,path TEXT,count INTEGER);CREATE TABLE map_images_points(image_id INTEGER,point_id INTEGER,image_n INTEGER);CREATE TEMP TABLE mvp_query(id INTEGER,dist INTEGER);')
for i in range(60):
 db.execute('INSERT INTO images VALUES(?,?,?)',(i,'image'+str(i),1));db.execute('INSERT INTO map_images_points VALUES(?,?,?)',(i,i,0));db.execute('INSERT INTO temp.mvp_query VALUES(?,0)',(i,))
assert len(db.execute(query,{'limit':10}).fetchall())==10
print('PASS explicit LIMIT10 returns10of60equally distant candidates; capped result is API policy, not complete radius enumeration')
assert len(db.execute(query,{'limit':60}).fetchall())==60
print('PASS sufficient LIMIT retains all60tied images')
db.execute('INSERT INTO map_images_points VALUES(0,100,9)');db.execute('INSERT INTO temp.mvp_query VALUES(100,0)')
assert len(db.execute(query,{'limit':60}).fetchall())==60
print('PASS multiple matching frame points collapse to one image row; not timeline or full-video identity')
db.execute('DELETE FROM temp.mvp_query');assert db.execute(query,{'limit':60}).fetchall()==[]
print('PASS empty candidate table returns empty query result')
print('RESULT 4 passed; unchanged original SQL via PythonSQLite3; synthetic point-query table, not original MVP C++ or image pipeline')
