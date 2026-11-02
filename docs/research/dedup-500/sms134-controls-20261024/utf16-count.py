import subprocess,sys,tempfile,json
from pathlib import Path
from lxml import etree
root=Path('docs/research/dedup-500/sms134-original-tests-20261024').resolve()
with tempfile.TemporaryDirectory(prefix='sms134-utf16-',dir='.dedup-work') as t:
 folder=Path(t).resolve();out=folder/'output.xml';log=folder/'run.log';r=subprocess.run([sys.executable,str(root/'dedupe_texts.py'),'-i',str(root/'tests/test_cases/utf16_encoding.xml'),'-o',str(out),'-l',str(log),'--fix-utf16'],capture_output=True,text=True);assert r.returncode==0,(r.stdout,r.stderr);tree=etree.parse(str(out));expected=etree.parse(str(root/'tests/test_cases/utf16_encoding_corrected.xml'));assert tree.getroot().attrib['count']=='1' and len(tree.getroot())==1;assert tree.getroot()[0].attrib['body']==expected.getroot()[0].attrib['body'];assert '😂' in tree.getroot()[0].attrib['body'];result={'controls_passed':1,'scope':'Unchanged original CLI full process;owned upstream UTF16 fixture;actual lxml6.0.2;no real backup restore','input_count':2,'output_count':1,'unicode_preserved':True,'exit_code':r.returncode,'stdout':r.stdout}
Path(__file__).with_name('utf16-count-results.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
