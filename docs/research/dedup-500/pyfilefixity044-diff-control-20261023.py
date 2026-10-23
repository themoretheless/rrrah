import json,sys,tempfile
from pathlib import Path
cache=Path('.dedup-work/pyfilefixity044-20261023').resolve()
for p in [cache/'dependencies',cache/'pyFileFixity',cache]:sys.path.insert(0,str(p))
from pyFileFixity import resiliency_tester as module
with tempfile.TemporaryDirectory(prefix='fixity-diff-review-') as folder:
    root=Path(folder);original=root/'original';output=root/'output';original.mkdir();output.mkdir()
    a=original/'x.bin';b=output/'x.bin';a.write_bytes(b'a');b.write_bytes(b'ab')
    same_block=module.diff_bytes_files(str(a),str(b))
    assert same_block==(0,1)
    stats=module.compute_diff_stats(str(original),str(original),str(output))
    assert stats['error']==0 and stats['diff_count']==(1,1)
    b.write_bytes(b'a');(output/'extra.bin').write_bytes(b'extra')
    extra=module.diff_count_dir(str(original),str(output));assert extra==(0,1)
    a.write_bytes(b'a'*9);b.write_bytes(b'a')
    try:module.diff_bytes_files(str(a),str(b),blocksize=4)
    except AttributeError as exc:exception=str(exc)
    else:raise AssertionError('Expected invalid file.fstat call')
    print(json.dumps({'same_block_unequal_lengths':same_block,'reported_byte_error_percent':stats['error'],'reported_file_difference':stats['diff_count'],'extra_file_ignored':extra,'cross_block_length_error':exception},indent=2))
