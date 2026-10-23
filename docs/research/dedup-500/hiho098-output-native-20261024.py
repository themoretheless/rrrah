from pathlib import Path
import subprocess,json,shutil,tempfile
root=Path.cwd();b=root/'.dedup-work/hiho098-runtime-20261024';j=next(b.glob('*/Contents/Home'));f=root/'docs/research/dedup-500/evidence/sonalgoyal--hiho';cp=str(b/'classes')+':'+str(b/'jars/*');sources=[]
for local in ['98-AppendSequenceFileOutputFormat.java','99-AppendTextOutputFormat.java','100-NoKeyOnlyValueOutputFormat.java']:
 p=b/'src'/local.split('-',1)[1];shutil.copyfile(f/local,p);sources.append(str(p))
steps=[]
with tempfile.TemporaryDirectory(prefix='hiho098-output-',dir=root/'.dedup-work') as tmp:
 for args in [[str(j/'bin/javac'),'-cp',cp,'-d',str(b/'classes'),*sources,str(b/'OutputBoundaryControl.java')],[str(j/'bin/java'),'-cp',cp,'co.nubetech.hiho.mapreduce.lib.output.OutputBoundaryControl',tmp]]:
  p=subprocess.run(args,capture_output=True,text=True,timeout=30);steps.append(dict(args=args,returncode=p.returncode,stdout=p.stdout,stderr=p.stderr));print(p.stdout,p.stderr)
  if p.returncode:break
(root/'docs/research/dedup-500/hiho098-output-native-controls-20261024.json').write_text(json.dumps(dict(scope='Original writer/naming methods;8author controls with byte buffers/owned failing stream and owned temporary sparse directories;realHadoop0.20.2; no commit, cluster, FTP or overwrite of existing user files',commands=steps),indent=2)+'\n');assert all(x['returncode']==0 for x in steps)
