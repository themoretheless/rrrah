from pathlib import Path
import tempfile,subprocess,json,shutil
root=Path.cwd();b=root/'.dedup-work/hiho098-runtime-20261024';j=next(b.glob('*/Contents/Home'));f=root/'docs/research/dedup-500/evidence/sonalgoyal--hiho';cp=str(b/'classes')+':'+str(b/'jars/*');sources=[]
for local in ['91-FileStreamInputFormat.java','92-FileStreamRecordReader.java']:
 p=b/'src'/local.split('-',1)[1];shutil.copyfile(f/local,p);sources.append(str(p))
steps=[]
with tempfile.TemporaryDirectory(prefix='hiho098-stream-',dir=root/'.dedup-work') as temp:
 for args in [[str(j/'bin/javac'),'-cp',cp,'-d',str(b/'classes'),*sources,str(b/'StreamBoundaryControl.java')],[str(j/'bin/java'),'-cp',cp,'StreamBoundaryControl',temp]]:
  p=subprocess.run(args,capture_output=True,text=True,timeout=30);steps.append(dict(args=args,returncode=p.returncode,stdout=p.stdout,stderr=p.stderr));print(p.stdout,p.stderr)
  if p.returncode:break
(root/'docs/research/dedup-500/hiho098-stream-native-controls-20261024.json').write_text(json.dumps(dict(scope='Original reader methods, actual local Hadoop filesystem and owned temporary files;3payload/EOF positive controls+1basename-key observation; no cluster/original test invocation',author_setup_correction='Initial harness used unsupported Hadoop0.20 Path(URI) constructor and did not compile. Changed own harness to Path(uri.toString()); original source unchanged.',initial_attempt=json.loads((root/'.dedup-work/hiho098-stream-initial-uri-constructor-20261024.json').read_text()),commands=steps),indent=2)+'\n');assert all(x['returncode']==0 for x in steps)
