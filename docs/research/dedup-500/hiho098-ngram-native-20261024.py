from pathlib import Path
import subprocess,json,shutil
root=Path.cwd();b=root/'.dedup-work/hiho098-runtime-20261024';j=next(b.glob('*/Contents/Home'));f=root/'docs/research/dedup-500/evidence/sonalgoyal--hiho';cp=str(b/'classes')+':'+str(b/'jars/*');sources=[]
for local in ['66-NGramMapper.java','67-NGramReducer.java','69-ScoreMapper.java','70-ScoreReducer.java','72-ValuePair.java','74-TestNGramMapper.java','75-TestNGramReducer.java','77-TestScoreMapper.java','78-TestScoreReducer.java']:
 p=b/'src'/local.split('-',1)[1];shutil.copyfile(f/local,p);sources.append(str(p))
steps=[]
for args in [[str(j/'bin/javac'),'-cp',cp,'-d',str(b/'classes'),*sources,str(b/'NGramBoundaryControl.java')],[str(j/'bin/java'),'--add-opens=java.base/java.lang=ALL-UNNAMED','-cp',cp,'org.junit.runner.JUnitCore',*['co.nubetech.hiho.similarity.ngram.'+x for x in ['TestNGramMapper','TestNGramReducer','TestScoreMapper','TestScoreReducer']]],[str(j/'bin/java'),'--add-opens=java.base/java.lang=ALL-UNNAMED','-cp',cp,'NGramBoundaryControl']]:
 p=subprocess.run(args,capture_output=True,text=True,timeout=30);steps.append(dict(args=args,returncode=p.returncode,stdout=p.stdout,stderr=p.stderr));print(p.stdout,p.stderr)
 if p.returncode:break
(root/'docs/research/dedup-500/hiho098-ngram-native-controls-20261024.json').write_text(json.dumps(dict(scope='8originalJUnitmethods plus4authorboundarycontrols; unchanged selected original5Java classes; realHadoop0.20.2/Mockito1.8.1; mocked recording contexts; no full shuffle/cluster',commands=steps),indent=2)+'\n');assert all(x['returncode']==0 for x in steps)
