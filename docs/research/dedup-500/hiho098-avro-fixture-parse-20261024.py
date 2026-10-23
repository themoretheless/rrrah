"""Bounded complete structural decode of the pinned tiny int/string null-codec fixture."""
from pathlib import Path
import io,json,hashlib
root=Path.cwd();p=root/'docs/research/dedup-500/evidence/sonalgoyal--hiho/138-data.avro';raw=p.read_bytes();s=io.BytesIO(raw)
def read(stream,count):
 assert 0<=count<=len(raw)
 b=stream.read(count);assert len(b)==count;return b
def number(stream):
 value=0
 for shift in range(0,70,7):
  b=read(stream,1)[0];value|=(b&127)<<shift
  if not b&128:return (value>>1)^-(value&1)
 raise AssertionError('oversized varint')
def blob(stream):return read(stream,number(stream))
assert read(s,4)==b'Obj\x01';metadata={}
while True:
 count=number(s)
 if count==0:break
 assert 0<count<=4
 for _ in range(count):
  key=blob(s).decode();metadata[key]=blob(s)
assert set(metadata)=={'avro.schema'}
schema=json.loads(metadata['avro.schema']);assert [(f['name'],f['type']) for f in schema['fields']]==[('intColumn','int'),('stringColumn','string')]
sync=read(s,16);blocks=[];records=[]
while s.tell()<len(raw):
 count=number(s);size=number(s);assert count==1 and size==5
 block=io.BytesIO(read(s,size));rows=[]
 for _ in range(count):rows.append(dict(intColumn=number(block),stringColumn=blob(block).decode()))
 assert block.tell()==size and read(s,16)==sync
 blocks.append(dict(count=count,payload_bytes=size));records+=rows
assert s.tell()==len(raw)==180 and records==[dict(intColumn=12,stringColumn='sam')]
result=dict(sha256=hashlib.sha256(raw).hexdigest(),bytes=len(raw),schema=schema,metadata_keys=list(metadata),codec='null (absent codec metadata)',sync_hex=sync.hex(),blocks=blocks,records=records,consumed=s.tell(),scope='All180bytes accounted by header,metadata,sync,varints,blockpayload and repeatedsync; fixture-specific structural decoder, not a general Avro implementation/upstream test')
(root/'docs/research/dedup-500/hiho098-avro-fixture-structure-20261024.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
