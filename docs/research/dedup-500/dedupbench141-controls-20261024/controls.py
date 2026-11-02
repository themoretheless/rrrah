import runpy,tempfile,json
from pathlib import Path
source=Path('docs/research/dedup-500/evidence/UWASL--dedup-bench/02-calculate_file_similarities.py');m=runpy.run_path(str(source),run_name='owned_module');f=m['calc_similarity']
with tempfile.TemporaryDirectory(prefix='dedupbench141-',dir='.dedup-work') as t:
 root=Path(t);a=root/'a';b=root/'b'
 def pair(x,y):a.write_bytes(x);b.write_bytes(y);return f(str(a),str(b))
 ascii_score=pair(b'abc',b'abc');assert ascii_score==1
 unicode_score=pair(('é'*4097).encode(),('é'*4097).encode());assert unicode_score==0.5
 newline_score=pair(b'a\r\n',b'a\n');assert newline_score==2/3
 try:pair(b'',b'');raise AssertionError('empty accepted')
 except ZeroDivisionError:pass
 try:pair(b'\xff',b'\xff');raise AssertionError('binary accepted')
 except UnicodeDecodeError:pass
result={'controls_passed':5,'scope':'Original full module imported without CLI main;actual owned filesystem inputs;UTF8 macOS Python text IO;no C++ chunker or benchmark dataset execution','findings':['ASCII identical scores1.','Byte-identical UTF8 unicode files score0.5 because character numerator/byte denominator.','CRLF versus LF normalized in text IO;score2/3,not byte-position equality.','Two empty files raise ZeroDivisionError.','Identical nonUTF8 byte ff raises UnicodeDecodeError.'],'scores':{'ascii':ascii_score,'unicode':unicode_score,'newline':newline_score}};Path(__file__).with_name('results.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
