import io,json,sys
from pathlib import Path
root=Path('.dedup-work/pyfilefixity044-20261023').resolve()
sys.path.insert(0,str(root/'dependencies'))
sys.path.insert(0,str(root/'pyFileFixity'))
sys.path.insert(0,str(root))
from pyFileFixity import replication_repair
results={}
for name,contents in [('two_two_tie',[b'A',b'A',b'B',b'B']),('later_file_tail',[b'ab',b'ab',b'abTAIL'])]:
    out=io.BytesIO()
    code,message=replication_repair.majority_vote_byte_scan('fixture.bin',[io.BytesIO(x) for x in contents],out,blocksize=2)
    results[name]={'return':code,'message':message,'output':out.getvalue().decode()}
assert results['two_two_tie']=={'return':0,'message':None,'output':'A'}
assert results['later_file_tail']=={'return':0,'message':None,'output':'ab'}
try:
    replication_repair.majority_vote_byte_scan('fixture.bin',[io.BytesIO(b'abc')],io.BytesIO())
except UnboundLocalError as exc:
    results['single_file_stream_output']={'exception':type(exc).__name__,'message':str(exc)}
else:raise AssertionError('Expected stream output branch failure')
print(json.dumps(results,indent=2))
