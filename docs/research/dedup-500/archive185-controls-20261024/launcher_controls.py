"""Original pinned POSIX launcher; owned filesystem and recording-only Java stub.

No JVM, indexing, archives, external network or user data touched.
"""
from pathlib import Path
import json,os,subprocess,sys,tempfile
base=Path(__file__).resolve().parent;source=base.parent/'evidence/Landsbokasafn--deduplicator/remaining-pinned/deduplicator-dist/src/main/bin/index';results=[]
with tempfile.TemporaryDirectory(prefix='archive185-shell-') as root:
 root=Path(root);bin=root/'java/bin';bin.mkdir(parents=True);stub=bin/'java';capture=root/'capture.json';stub.write_text('#!'+sys.executable+'\nimport json,os,sys\nfrom pathlib import Path\nPath(os.environ["OWNED_CAPTURE"]).write_text(json.dumps({"argv":sys.argv[1:],"cwd":os.getcwd(),"classpath":os.environ.get("CLASSPATH")}))\n');stub.chmod(0o755)
 for case,folder,args in [('plain','plain',['source','target']),('argument_space','plain',['source with space','target']),('home_space','home with space',['source','target'])]:
  home=root/folder;(home/'lib').mkdir(parents=True,exist_ok=True);(home/'lib/dummy.jar').touch();env=os.environ.copy();env.update(JAVA_HOME=str(root/'java'),JAVACMD=str(stub),DEDUPLICATOR_HOME=str(home),JAVA_OPTS='-Xmx256m',JMX_OPTS='',CLASS_MAIN='is.landsbokasafn.deduplicator.indexer.IndexingLauncher',OWNED_CAPTURE=str(capture));env.pop('CP',None)
  p=subprocess.run(['sh',str(source),*args],cwd=root,env=env,text=True,capture_output=True,timeout=20);got=json.loads(capture.read_text());assert p.returncode==0
  if case=='plain':assert got['argv'][-2:]==args and got['cwd']==str(home.resolve())
  if case=='argument_space':assert got['argv'][-4:]==['source','with','space','target']
  if case=='home_space':assert got['cwd']==str(root.resolve()) and p.stderr
  results.append({'case':case,'exit_code':p.returncode,'requested_args':args,'received_tail':got['argv'][-len(args):] if case!='argument_space' else got['argv'][-4:],'expected_directory_entered':got['cwd']==str(home.resolve()),'stderr_present':bool(p.stderr)})
p={'scope':__doc__,'authored_controls_passed':len(results),'results':results,'limits':'Real POSIX shell and original launcher only;Java recording stub does not qualify JVM/classpath loading/indexing/Maven.'};(base/'launcher-results.json').write_text(json.dumps(p,indent=2)+'\n');print(json.dumps(p,indent=2))
