"""Unchanged analyzer with actual Gensim/NLTK; runner helpers/flag adapter never do HTTP."""
from pathlib import Path
import sys,os,json,tempfile,contextlib,io,ast,argparse,subprocess,platform,types,hashlib,math,csv
R=Path(__file__).resolve().parents[3];W=Path('/private/tmp/rrrah-code33-native-20261015');sys.path[:0]=[str(W/'modules'),str(W)];os.environ['NLTK_DATA']=str(W/'nltk_data')
import duplicate_code_detection as d
import gensim,nltk,astor,numpy,scipy
assert gensim.__version__=='4.3.3' and nltk.__version__=='3.9.1' and numpy.__version__=='1.26.4' and scipy.__version__=='1.12.0'
OBS={}
def equal(a,b):assert a==b,(a,b)
def raises(kind,fn):
 try:fn()
 except kind:return
 raise AssertionError('expected '+kind.__name__)
def docs(root,texts=None,suffix='c'):
 texts=texts or ['alpha beta gamma\n','alpha beta gamma\n','delta epsilon zeta\n'];paths=[]
 for i,text in enumerate(texts):
  p=root/f'{i}.{suffix}';p.write_text(text);paths.append(str(p))
 return paths
def call(root,paths,**kw):
 model=root/'model';model.mkdir(exist_ok=True);old=tempfile.tempdir;tempfile.tempdir=str(model);out=io.StringIO();args=dict(fail_threshold=50,directories=None,files=paths,ignore_directories=[],ignore_files=[],json_output=True,project_root_dir=str(root),file_extensions=['c','py'],ignore_threshold=0,only_code=False,csv_output='',show_loc=False);args.update(kw)
 try:
  with contextlib.redirect_stdout(out):code,result=d.run(**args)
 finally:tempfile.tempdir=old
 return code,result,out.getvalue()
def identical_pair_zero(root):
 paths=docs(root,['alpha beta gamma\n']*2);code,result,_=call(root,paths);equal(code,d.ReturnCode.SUCCESS);equal(result['0.c']['1.c'],0.0);OBS['identical_two_docs_score']=result['0.c']['1.c']
def corpus_adds_score(root):
 paths=docs(root);code,result,_=call(root,paths);equal(code,d.ReturnCode.THRESHOLD_EXCEEDED);equal(result['0.c']['1.c'],100.0);OBS['same_pair_with_third_document_score']=result['0.c']['1.c']
def token_order_ignored(root):
 paths=docs(root,['alpha beta gamma\n','gamma beta alpha\n','delta epsilon zeta\n']);assert Path(paths[0]).read_bytes()!=Path(paths[1]).read_bytes();code,result,_=call(root,paths);equal(code,d.ReturnCode.THRESHOLD_EXCEEDED);equal(result['0.c']['1.c'],100.0)
def filter_hides_failure(root):
 paths=docs(root,['shared one uniquea\n','shared one uniqueb\n','other only distinct\n']);code,result,_=call(root,paths,fail_threshold=20);score=result['0.c']['1.c'];expected=100*(2*math.log2(3/2)**2)/(2*math.log2(3/2)**2+math.log2(3)**2);equal(score,round(expected,2));assert 20<score<30;equal(code,d.ReturnCode.THRESHOLD_EXCEEDED);code,result,_=call(root,paths,fail_threshold=20,ignore_threshold=30);equal(code,d.ReturnCode.SUCCESS);equal(result,{});OBS['filter']=dict(score=score,fail_threshold=20,ignore_threshold=30,filtered_exit='SUCCESS')
def invalid_python_success(root):
 paths=docs(root,['def invalid(:\n']*2,'py');code,result,text=call(root,paths,only_code=True);equal(code,d.ReturnCode.SUCCESS);equal(result,{});equal(text.count('ERROR: Failed to open file'),2);raises(json.JSONDecodeError,lambda:json.loads(text))
def stripped_empty_body_invalid(root):
 cleaned=d.remove_comments_and_docstrings('def f():\n    "doc"\n');equal(cleaned,'def f():\n');raises(SyntaxError,lambda:ast.parse(cleaned))
def canonical_alias_admission(root):
 p=docs(root,['alpha beta\n'])[0];rel=os.path.relpath(p,Path.cwd());code,result,_=call(root,[p,rel]);equal(code,d.ReturnCode.SUCCESS);equal(result,{})
def relative_ignore_misses_absolute(root):
 paths=docs(root);relative=os.path.relpath(paths[0],Path.cwd());code,result,_=call(root,paths,ignore_files=[relative]);equal(code,d.ReturnCode.THRESHOLD_EXCEEDED);assert '0.c' in result
 code,result,_=call(root,paths,ignore_files=[paths[0]]);equal(code,d.ReturnCode.SUCCESS);assert '0.c' not in result
def json_false_true(root):
 paths=docs(root);oldargv=sys.argv;oldtmp=tempfile.tempdir;model=root/'model';model.mkdir();tempfile.tempdir=str(model);out=io.StringIO()
 try:
  sys.argv=['owned','-f',*paths,'--json','False','--fail-threshold','101']
  with contextlib.redirect_stdout(out):code,result=d.main()
 finally:sys.argv=oldargv;tempfile.tempdir=oldtmp
 equal(code,d.ReturnCode.SUCCESS);equal(json.loads(out.getvalue()),result);assert 'Code duplication probability' not in out.getvalue()
def console_exit_contract(root):
 paths=docs(root);tmp=root/'model';tmp.mkdir();env=dict(os.environ,PYTHONDONTWRITEBYTECODE='1',PYTHONPATH=os.pathsep.join([str(W/'modules'),str(W)]),NLTK_DATA=str(W/'nltk_data'),TMPDIR=str(tmp),OPENBLAS_NUM_THREADS='1',OMP_NUM_THREADS='1',MKL_NUM_THREADS='1');args=['-f',*paths,'--json','True','--fail-threshold','101']
 direct=subprocess.run([sys.executable,str(W/'duplicate_code_detection.py'),*args],cwd=W,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE);equal(direct.returncode,0)
 wrapper=subprocess.run([sys.executable,'-c','import sys,duplicate_code_detection as d;sys.exit(d.main())',*args],cwd=W,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE);equal(wrapper.returncode,1);assert b'ReturnCode.SUCCESS' in wrapper.stderr;equal(json.loads(direct.stdout),json.loads(wrapper.stdout));OBS['console_exit']=dict(original_standalone=0,explicit_standard_entrypoint_wrapper=1,scope='Actualmain, explicitstandardconsolewrapper; no installedsetuptoolspackage test')
def model_overwrites_fixed_name(root):
 paths=docs(root);model=root/'model';model.mkdir();marker=model/'.0';marker.write_bytes(b'OWNED previous unrelated data');call(root,paths);assert marker.read_bytes()!=b'OWNED previous unrelated data';OBS['model_path']=dict(owned_existing_dot0_overwritten=True)
def csv_directed_pairs_loc(root):
 paths=docs(root);out=root/'report.csv';code,result,_=call(root,paths,fail_threshold=101,ignore_threshold=50,csv_output=str(out),show_loc=True);equal(code,d.ReturnCode.SUCCESS);rows=list(csv.reader(out.open()));equal(rows[0],['File A','#LoC A','File B','#LoC B','Similarity']);equal(len(rows),3);equal({(r[0],r[2]) for r in rows[1:]},{('0.c','1.c'),('1.c','0.c')});assert all(r[1]==r[3]=='1' and r[4]=='100.0' for r in rows[1:])
# Pure runner functions are selected byte-original AST bodies, not the network-posting action.
runner_tree=ast.parse((W/'run_action.py').read_text());helpers=['make_markdown_table','get_markdown_link','get_warning','similarities_to_markdown','split_and_trim','to_absolute_path'];runner={'os':os,'WARNING_SUFFIX':' ⚠️'};exec(compile(ast.Module(body=[n for n in runner_tree.body if isinstance(n,ast.FunctionDef) and n.name in helpers],type_ignores=[]),'upstream-runner-helpers','exec'),runner)
def markdown_premature_limit(root):
 rows=[['File','Similarity']]+[[f'file_{i:04}',50] for i in range(1000)];text=runner['make_markdown_table'](rows);assert 'Results were omitted' in text;assert len(text)<65000;assert 0<text.count('file_')<1000;OBS['markdown']=dict(input_rows=1000,output_rows=text.count('file_'),output_chars=len(text),declared_max=65000)
def markdown_unescaped(root):
 link=runner['get_markdown_link']('a b#c.py','https://example.invalid/');equal(link,'[a b#c.py](https://example.invalid/a b#c.py)');text=runner['similarities_to_markdown']({'<OWNED>':{'a|b.py':90}},'https://example.invalid/',50);assert '<summary><OWNED></summary>' in text and 'a|b.py' in text
class OwnedStop(Exception):pass
def action_false_true(root):
 captured=[]
 class Capture:
  @staticmethod
  def run(*args):captured.append(args[9]);raise OwnedStop()
 class NoHTTP:
  def __getattr__(self,name):raise AssertionError('HTTP forbidden '+name)
 values=dict(INPUT_FAIL_ABOVE='50',INPUT_DIRECTORIES=str(root),INPUT_IGNORE_DIRECTORIES='',INPUT_PROJECT_ROOT_DIR=str(root),INPUT_FILE_EXTENSIONS='py',INPUT_IGNORE_BELOW='0',INPUT_ONLY_CODE='false');ns=dict(runner,argparse=argparse,sys=sys,os=types.SimpleNamespace(path=os.path,environ=values),duplicate_code_detection=Capture,requests=NoHTTP());exec(compile(ast.Module(body=[n for n in runner_tree.body if isinstance(n,ast.FunctionDef) and n.name=='main'],type_ignores=[]),'upstream-runner-main-preHTTP','exec'),ns);old=sys.argv
 try:sys.argv=['owned','--pull-request-id','1'];raises(OwnedStop,ns['main'])
 finally:sys.argv=old
 equal(captured,[True]);OBS['action_false']=dict(input='false',analyzer_only_code=True,scope='ASToriginalrunner main, explicitanalyzerstop beforeHTTP/tokenreads/messagewrites; noGitHubAction execution')
def extension_case_admission(root):
 for name in ['a.py','b.PY','c.hpp','d.c']:(root/name).write_text('code')
 actual={Path(p).name for p in d.get_all_source_code_from_directory(str(root),d.source_code_file_extensions)};equal(actual,{'a.py','d.c'})
if __name__=='__main__':
 cases=[identical_pair_zero,corpus_adds_score,token_order_ignored,filter_hides_failure,invalid_python_success,stripped_empty_body_invalid,canonical_alias_admission,relative_ignore_misses_absolute,json_false_true,console_exit_contract,model_overwrites_fixed_name,csv_directed_pairs_loc,markdown_premature_limit,markdown_unescaped,action_false_true,extension_case_admission];passed=[]
 for f in cases:
  with tempfile.TemporaryDirectory(prefix='rrrah-code33-owned-',dir='/private/tmp') as td:f(Path(td))
  passed.append(f.__name__);print('PASS',f.__name__,flush=True)
 print(json.dumps(dict(status='passed16_scoped_native_analyzer_runner_controls',cases=len(passed),case_names=passed,observations=OBS,python=platform.python_version(),gensim=gensim.__version__,numpy=numpy.__version__,scipy=scipy.__version__,nltk=nltk.__version__,source_sha256={name:hashlib.sha256((W/name).read_bytes()).hexdigest() for name in ['duplicate_code_detection.py','run_action.py']},scope='13 actualanalyzer/normalizer/CLI/helper controls;2 selectedpure runnerhelpers;1 ASTselectedrunnermain explicitstopadapter beforeHTTP. Real Gensim/NLTK/astor/NumPy/SciPy/EnglishPunkt tables, owned modelTMP/1thread resource settings. No HTTP/Git/remotecomments/actions/installedpackage/release/corpus/platform/scale/performance/adoption proof.')))
