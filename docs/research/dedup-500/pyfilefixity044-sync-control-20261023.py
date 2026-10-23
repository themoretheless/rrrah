import io,json,sys,tempfile
from pathlib import Path
cache=Path('.dedup-work/pyfilefixity044-20261023').resolve()
for p in [cache/'dependencies',cache/'pyFileFixity',cache]:sys.path.insert(0,str(p))
from pyFileFixity import rfigc,replication_repair
with tempfile.TemporaryDirectory(prefix='fixity-sync-review-') as temporary:
    root=Path(temporary);trusted=root/'trusted';trusted.mkdir();(trusted/'x.bin').write_bytes(b'GOOD')
    database=root/'database.csv'
    assert rfigc.main(['-i',str(trusted),'-d',str(database),'-g','--silent'])==0
    copies=[]
    for i in range(3):
        folder=root/f'copy{i}';folder.mkdir();(folder/'x.bin').write_bytes(b'FAIL');copies.append(str(folder))
    output=root/'output';output.mkdir()
    try:replication_repair.synchronize_files(copies,str(output),database=str(database),ptee=io.StringIO())
    except UnboundLocalError as error:
        result={'exception':type(error).__name__,'message':str(error),'output_bytes':(output/'x.bin').read_bytes().decode()}
    else:raise AssertionError('Expected missing report row failure')
    assert result['output_bytes']=='FAIL'
    print(json.dumps(result,indent=2))
